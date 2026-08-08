# Phase 2: Async Wrapper

## Overview

This phase creates an async wrapper that drives the FSM execution, handles AI calls, tool executions, pause/resume, and abort. The wrapper bridges the synchronous FSM with the async I/O operations.

**Scope:**
- Create `FsmToolLoop` struct that wraps the FSM
- Implement main loop that feeds inputs to FSM and executes actions
- Handle AI calls via `OpenAiClient`
- Handle tool executions via MCP clients
- Handle pause/resume via `pause_notify`
- Handle abort via `CancellationToken`
- Integrate with existing `ChatManager` for state management

**Out of Scope:**
- DB synchronization (Phase 3)
- Helper FSMs for built-in tools (Phase 6)
- Replacement of existing `tool_loop()` function (Phase 4)

## Files to Create

### 1. `packages/rhd_chat/src/tools/fsm_wrapper.rs` (NEW FILE)

**Purpose:** Async wrapper that drives FSM execution and handles I/O operations.

**Complete Implementation:**

```rust
use std::sync::Arc;

use rhd_ai::client::{OpenAiClient, RawLogger, ToolCall};
use rhd_ai::ToolDefinition;
use rhd_fsm::{ToolLoopAction, ToolLoopFsm, ToolLoopFsmEvent, ToolLoopInput};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::ToolResult;
use tokio::sync::{broadcast, Mutex, Notify};
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLoggers;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

use super::messages::build_chat_messages_for_tools;
use super::tool_loop::{collect_tools_from_projects, execute_tool_call, ToolLoopResult};
use super::utils::extract_mcp_id_from_tool_name;

/// Async wrapper that drives the FSM-based tool loop
pub struct FsmToolLoop<'a, P: ProjectProvider> {
    fsm: ToolLoopFsm,
    manager: &'a ChatManager<P>,
    chat_id: i64,
    model: String,
    api_model: String,
    client: &'a OpenAiClient,
    mcp_clients: Vec<(String, String, Arc<McpClient>)>,
    cancel_token: CancellationToken,
    event_sender: broadcast::Sender<ChatEvent>,
    pause_notify: Arc<Notify>,
    iterations: u32,
    max_iterations: u32,
    loggers: Option<ChatLoggers>,
    template_loader: &'a TemplateLoaderRef,
}

impl<'a, P: ProjectProvider> FsmToolLoop<'a, P> {
    /// Create a new FSM tool loop wrapper
    pub fn new(
        manager: &'a ChatManager<P>,
        chat_id: i64,
        model: String,
        api_model: String,
        client: &'a OpenAiClient,
        mcp_clients: Vec<(String, String, Arc<McpClient>)>,
        cancel_token: CancellationToken,
        event_sender: broadcast::Sender<ChatEvent>,
        pause_notify: Arc<Notify>,
        max_iterations: u32,
        loggers: Option<ChatLoggers>,
        template_loader: &'a TemplateLoaderRef,
    ) -> Self {
        // Get initial message ID counter from DB
        let initial_message_id = manager
            .db()
            .get_next_message_id(chat_id)
            .unwrap_or(1_000_000);

        let fsm = ToolLoopFsm::with_message_id_counter(initial_message_id);

        Self {
            fsm,
            manager,
            chat_id,
            model,
            api_model,
            client,
            mcp_clients,
            cancel_token,
            event_sender,
            pause_notify,
            iterations: 0,
            max_iterations,
            loggers,
            template_loader,
        }
    }

    /// Run the tool loop until completion, pause, or abort
    pub async fn run(&mut self) -> Result<ToolLoopResult, ChatError> {
        // Load initial messages from DB
        self.load_initial_messages()?;

        // Load tools
        self.load_tools().await?;

        // Start the FSM
        let mut inputs = vec![ToolLoopInput::Run];
        let mut actions = self.fsm.run(&mut inputs)?;

        loop {
            // Check for abort
            if self.cancel_token.is_cancelled() {
                self.handle_abort().await?;
                return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: self.model.clone(),
                }));
            }

            // Check for max iterations
            if self.iterations >= self.max_iterations {
                return Err(ChatError::Ai(rhd_ai::client::AiError::Api {
                    model: self.model.clone(),
                    status: 0,
                    body: format!("max tool iterations ({}) exceeded", self.max_iterations),
                }));
            }

            // Process actions
            for action in actions {
                match action {
                    ToolLoopAction::SendToAi { messages, tools } => {
                        let result = self.handle_send_to_ai(&messages, &tools).await?;
                        let mut next_inputs = vec![ToolLoopInput::ProvideAiResponse {
                            content: result.content,
                            thinking_content: result.thinking_content,
                            tool_calls: result.tool_calls,
                            finish_reason: result.finish_reason.unwrap_or_else(|| "stop".to_string()),
                        }];
                        actions = self.fsm.run(&mut next_inputs)?;
                        continue;
                    }
                    ToolLoopAction::ExecuteToolCall { tool_call } => {
                        let result = self.handle_execute_tool_call(&tool_call).await?;
                        let mut next_inputs = vec![ToolLoopInput::ProvideToolResult {
                            tool_call_id: tool_call.id.clone(),
                            result,
                        }];
                        actions = self.fsm.run(&mut next_inputs)?;
                        continue;
                    }
                    ToolLoopAction::Completed { message } => {
                        return Ok(ToolLoopResult::Completed { message_id: message.id });
                    }
                    ToolLoopAction::Paused => {
                        // Wait for resume signal
                        self.pause_notify.notified().await;
                        
                        // Check if we were aborted while paused
                        if self.cancel_token.is_cancelled() {
                            self.handle_abort().await?;
                            return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                                model: self.model.clone(),
                            }));
                        }
                        
                        // Resume the FSM
                        let mut next_inputs = vec![ToolLoopInput::Resume];
                        actions = self.fsm.run(&mut next_inputs)?;
                        continue;
                    }
                    ToolLoopAction::Aborted => {
                        self.handle_abort().await?;
                        return Err(ChatError::Aborted);
                    }
                    ToolLoopAction::GenerateToolCallId { tool_call_id } => {
                        // Tool call ID was already generated and emitted as event
                        // No action needed here
                        continue;
                    }
                    ToolLoopAction::Error { message } => {
                        return Err(ChatError::Internal(message));
                    }
                }
            }

            // If we get here, FSM is in a waiting state
            // Check if FSM is finished
            if self.fsm.is_finished() {
                break;
            }

            // Wait for external input (this shouldn't happen in normal flow)
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Err(ChatError::Internal("FSM finished without result".to_string()))
    }

    /// Load initial messages from DB into FSM
    fn load_initial_messages(&mut self) -> Result<(), ChatError> {
        let db_messages = self.manager.db().get_messages(self.chat_id)?;
        let chat_messages = build_chat_messages_for_tools(&db_messages);

        for msg in chat_messages {
            let fsm_message = rhd_fsm::ChatMessage {
                id: msg.id,
                role: msg.role.clone(),
                content: msg.content.clone(),
                thinking_content: msg.thinking_content.clone(),
                tool_calls: msg.tool_calls.as_ref().map(|tcs| {
                    tcs.iter()
                        .map(|tc| rhd_fsm::ToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        })
                        .collect()
                }),
            };
            let mut inputs = vec![ToolLoopInput::InsertMessage { message: fsm_message }];
            self.fsm.run(&mut inputs)?;
        }

        Ok(())
    }

    /// Load tools from projects into FSM
    async fn load_tools(&mut self) -> Result<(), ChatError> {
        let (tools, mcp_clients) = collect_tools_from_projects(
            &self.manager.db(),
            self.manager.project_provider(),
            self.chat_id,
            self.template_loader,
        )
        .await;

        self.mcp_clients = mcp_clients;

        for tool in tools {
            let fsm_tool = rhd_fsm::ToolDefinition {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                parameters: tool.function.parameters.clone(),
            };
            let mut inputs = vec![ToolLoopInput::AddTool { tool: fsm_tool }];
            self.fsm.run(&mut inputs)?;
        }

        Ok(())
    }

    /// Handle SendToAi action - call AI and return response
    async fn handle_send_to_ai(
        &mut self,
        messages: &[rhd_fsm::ChatMessage],
        tools: &[rhd_fsm::ToolDefinition],
    ) -> Result<AiResponse, ChatError> {
        // Inject pending role prompt if needed
        crate::projects::inject_pending_role_prompt(
            self.manager,
            self.chat_id,
            &self.event_sender,
            self.template_loader,
        )?;

        // Convert FSM messages to AI messages
        let chat_messages = build_chat_messages_for_tools(
            &messages
                .iter()
                .map(|m| rhd_db::ChatMessage {
                    id: m.id,
                    chat_id: self.chat_id,
                    role: m.role.clone(),
                    content: m.content.clone(),
                    thinking_content: m.thinking_content.clone(),
                    tool_calls: m.tool_calls.as_ref().map(|tcs| {
                        tcs.iter()
                            .map(|tc| rhd_db::ToolCall {
                                id: tc.id.clone(),
                                function: rhd_db::FunctionCall {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                },
                            })
                            .collect()
                    }),
                    model: None,
                    created_at: String::new(),
                })
                .collect::<Vec<_>>(),
        );

        // Convert FSM tools to AI tools
        let ai_tools: Vec<ToolDefinition> = tools
            .iter()
            .map(|t| ToolDefinition {
                tool_type: "function".to_string(),
                function: rhd_ai::FunctionDefinition {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect();

        // Accumulate streaming content
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_thinking = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let thinking_clone = accumulated_thinking.clone();
        let sender_for_closure = self.event_sender.clone();
        let chat_id = self.chat_id;

        let raw_log_ref = self
            .loggers
            .as_mut()
            .and_then(|l| l.raw_log.as_mut().map(|r| r as &mut dyn RawLogger));

        let result = self
            .client
            .chat_stream_with_tools(
                &self.api_model,
                &chat_messages,
                &ai_tools,
                self.cancel_token.clone(),
                move |chunk| {
                    let sender = sender_for_closure.clone();
                    let acc = accumulated_clone.clone();
                    let thinking_acc = thinking_clone.clone();
                    Box::pin(async move {
                        if let Some(content) = chunk.content {
                            if !content.is_empty() {
                                let _ = sender.send(ChatEvent::StreamChunk {
                                    chat_id,
                                    content: content.clone(),
                                });
                                let mut acc_guard = acc.lock().await;
                                acc_guard.push_str(&content);
                            }
                        }
                        if let Some(reasoning) = chunk.reasoning_content {
                            if !reasoning.is_empty() {
                                let _ = sender.send(ChatEvent::ThinkingChunk {
                                    chat_id,
                                    content: reasoning.clone(),
                                });
                                let mut thinking_guard = thinking_acc.lock().await;
                                thinking_guard.push_str(&reasoning);
                            }
                        }
                    })
                },
                raw_log_ref,
            )
            .await;

        // Handle AI errors
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                if let rhd_ai::client::AiError::Aborted { .. } = e {
                    let _ = self.event_sender.send(ChatEvent::StreamAborted { chat_id: self.chat_id });
                    self.manager.abort_chat(self.chat_id, Vec::new()).await;
                }
                return Err(ChatError::Ai(e));
            }
        };

        let final_content = accumulated_content.lock().await.clone();
        let full_thinking = accumulated_thinking.lock().await.clone();
        let finish_reason = result.finish_reason;

        // Log the response
        if let Some(ref mut l) = self.loggers {
            let reasoning = if full_thinking.is_empty() {
                None
            } else {
                Some(full_thinking.as_str())
            };
            l.chat_log.log_assistant_response(
                reasoning,
                &final_content,
                finish_reason.as_deref().unwrap_or("stop"),
                result.usage.as_ref(),
            );
        }

        // Convert AI tool calls to FSM tool calls
        let tool_calls: Vec<rhd_fsm::ToolCall> = result
            .tool_calls
            .into_iter()
            .map(|tc| rhd_fsm::ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect();

        // If there are tool calls, increment iteration counter
        if !tool_calls.is_empty() {
            self.iterations += 1;
        } else {
            // Log stream finished for final response
            if let Some(ref mut l) = self.loggers {
                l.chat_log.log_stream_finished(finish_reason.as_deref().unwrap_or("stop"), 0);
            }
        }

        Ok(AiResponse {
            content: if final_content.is_empty() {
                None
            } else {
                Some(final_content)
            },
            thinking_content: if full_thinking.is_empty() {
                None
            } else {
                Some(full_thinking)
            },
            tool_calls,
            finish_reason,
        })
    }

    /// Handle ExecuteToolCall action - execute tool and return result
    async fn handle_execute_tool_call(
        &mut self,
        tool_call: &rhd_fsm::ToolCall,
    ) -> Result<rhd_fsm::ToolResult, ChatError> {
        // Check if cancelled before starting tool
        if self.cancel_token.is_cancelled() {
            let aborted_result = rhd_fsm::ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: "Aborted".to_string(),
                is_error: true,
            };

            let _ = self.event_sender.send(ChatEvent::ToolCallCompleted {
                chat_id: self.chat_id,
                tool_call_id: tool_call.id.clone(),
                result: aborted_result.content.clone(),
                is_error: true,
            });

            return Ok(aborted_result);
        }

        // Log tool call
        if let Some(ref mut l) = self.loggers {
            l.chat_log.log_tool_call(&tool_call.name, &tool_call.id, &tool_call.arguments);
        }

        // Emit ToolCallStarted event
        let mcp_id = extract_mcp_id_from_tool_name(&tool_call.name);
        let _ = self.event_sender.send(ChatEvent::ToolCallStarted {
            chat_id: self.chat_id,
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
            mcp_id: mcp_id.clone(),
        });

        // Convert FSM tool call to AI tool call for execute_tool_call function
        let ai_tool_call = ToolCall {
            id: tool_call.id.clone(),
            function: rhd_ai::client::FunctionCall {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            },
        };

        // Execute the tool
        let (tool_result, _mcp_id) = execute_tool_call(
            self.manager,
            self.chat_id,
            &ai_tool_call,
            &self.mcp_clients,
            &self.event_sender,
        )
        .await;

        // Log tool result
        if let Some(ref mut l) = self.loggers {
            l.chat_log.log_tool_result(&tool_call.name, &tool_call.id, &tool_result.content);
            if let Some(ref mut raw_log) = l.raw_log {
                if let Some(ref raw_response) = tool_result.raw_response {
                    let raw_json = serde_json::to_string_pretty(raw_response)
                        .unwrap_or_else(|_| raw_response.to_string());
                    raw_log.log_tool_result_raw(&tool_call.name, &tool_call.id, &raw_json);
                }
            }
        }

        // Emit ToolCallCompleted event
        let _ = self.event_sender.send(ChatEvent::ToolCallCompleted {
            chat_id: self.chat_id,
            tool_call_id: tool_call.id.clone(),
            result: tool_result.content.clone(),
            is_error: tool_result.is_error.unwrap_or(false),
        });

        // Convert to FSM tool result
        Ok(rhd_fsm::ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: tool_result.content,
            is_error: tool_result.is_error.unwrap_or(false),
        })
    }

    /// Handle abort - clean up and transition to aborted state
    async fn handle_abort(&mut self) -> Result<(), ChatError> {
        if let Some(ref mut l) = self.loggers {
            l.chat_log.log_stream_error("aborted");
        }
        let _ = self.event_sender.send(ChatEvent::StreamAborted { chat_id: self.chat_id });
        self.manager.abort_chat(self.chat_id, Vec::new()).await;
        Ok(())
    }
}

/// Internal struct for AI response data
struct AiResponse {
    content: Option<String>,
    thinking_content: Option<String>,
    tool_calls: Vec<rhd_fsm::ToolCall>,
    finish_reason: Option<String>,
}
```

### 2. `packages/rhd_chat/src/tools/mod.rs`

**Modifications:**

Add export for the new module:

```rust
pub mod fsm_wrapper;
```

## Files to Modify

### 3. `packages/rhd_chat/Cargo.toml`

**Modifications:**

Add `rhd_fsm` dependency if not already present:

```toml
[dependencies]
rhd_fsm = { path = "../rhd_fsm" }
```

## Tests

### Unit Tests

Create `packages/rhd_chat/src/tools/tests/fsm_wrapper_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    // Tests will be added in Phase 5 (Integration Tests)
    // This phase focuses on the wrapper implementation
}
```

## Implementation Notes

1. **Message ID Synchronization**: The wrapper gets the initial message ID counter from the DB to ensure IDs don't conflict with existing messages.

2. **Event Flow**: The wrapper processes FSM actions in a loop:
   - `SendToAi` → Call AI client → Feed `ProvideAiResponse` back to FSM
   - `ExecuteToolCall` → Execute tool → Feed `ProvideToolResult` back to FSM
   - `Paused` → Wait on `pause_notify` → Feed `Resume` back to FSM
   - `Completed` → Return success
   - `Aborted` → Return error

3. **Pause/Resume**: When the FSM emits `Paused`, the wrapper waits on `pause_notify`. When resumed, it feeds `Resume` input back to the FSM.

4. **Abort Handling**: The wrapper checks `cancel_token` at multiple points:
   - Before processing each action
   - Before executing each tool
   - While waiting for resume

5. **Logging**: Logging is done in the wrapper based on FSM actions, not in the FSM itself. This keeps the FSM pure and synchronous.

6. **Built-in Tools**: For now, built-in tools (`rhd_set_todo_list`, `rhd_set_role`) are handled by the existing `execute_tool_call` function. Phase 6 will move this to helper FSMs.

7. **Error Handling**: Errors from AI calls and tool executions are converted to `ChatError` and propagated up.

## Dependencies

- This phase depends on Phase 1 (FSM Listener System) for the event system
- This phase must be completed before Phase 4 (Replace tool_loop)

## Success Criteria

- [ ] Wrapper can execute complete tool loop scenarios
- [ ] Pause/resume works correctly
- [ ] Abort works correctly
- [ ] Wrapper integrates with existing `ChatManager` state management
- [ ] Streaming events are emitted correctly
- [ ] Logging works correctly
- [ ] Code compiles without warnings
- [ ] Basic unit tests pass (to be expanded in Phase 5)
