# Phase 2: Tool Loop Integration

## Overview

This phase updates the tool loop to respect pause/abort states at the correct points: after AI call completes, before tool execution, and during tool execution. This ensures that pause and abort operations work correctly during different phases of the tool loop.

**Scope:**
- Add pause checks after AI call completes (before tool execution)
- Add abort checks before each tool execution
- Handle pause during AI call (store pending tool calls, exit loop)
- Handle pause during tool execution (complete current tools, insert results, exit loop)
- Handle abort during AI call (cancel request, discard response, emit event)
- Handle abort during tool execution (cancel non-finished tools, emit errors)
- Update `ChatManager` with helper methods for state queries

**Out of scope:**
- Message queue (Phase 3)
- Frontend changes (Phase 4)

**Dependencies:**
- Phase 1 (Backend State Machine) must be completed first

## Files to Modify

### 1. `packages/rhd_chat/src/tools/tool_loop.rs`

**Remove pause check from start of iteration:**

Current code (line 103):
```rust
manager.check_pause_state(chat_id, event_sender).await;
```

This should be removed from the start of the iteration. Pause checks will be added at more appropriate locations.

**Add pause check after AI call completes (before tool execution):**

After the AI call completes and before processing tool calls (around line 155), add:

```rust
// Check if we should pause after AI call completes
if let Some(state_info) = manager.get_stream_state(chat_id).await {
    if state_info.is_paused {
        // Store tool calls as pending and exit loop
        let pending_tool_calls: Vec<PendingToolCall> = result.tool_calls
            .iter()
            .map(|tc| PendingToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            })
            .collect();
        
        // Update state with pending tool calls
        manager.set_pending_tool_calls(chat_id, pending_tool_calls).await;
        
        // Exit tool loop, wait for resume
        return Err(ChatError::Paused);
    }
}
```

**Add abort check before each tool execution:**

Before executing each tool (around line 249), add:

```rust
// Check if aborted before executing tool
if let Some(state_info) = manager.get_stream_state(chat_id).await {
    if state_info.is_aborted {
        // This tool was aborted
        let aborted_result = ToolResult {
            content: "Aborted".to_string(),
            is_error: Some(true),
            raw_response: None,
        };
        
        // Emit ToolCallCompleted with error
        let _ = event_sender.send(ChatEvent::ToolCallCompleted {
            chat_id,
            tool_call_id: tool_call.id.clone(),
            result: aborted_result.content.clone(),
            is_error: true,
        });
        
        // Save tool result with error
        let tool_result_json = serde_json::json!({
            "toolCallId": tool_call.id,
            "name": tool_call.function.name,
            "result": aborted_result.content,
            "isError": true,
        })
        .to_string();
        
        let _ = manager.add_message_and_notify(
            chat_id,
            "tool",
            &tool_result_json,
            None,
            None,
            event_sender,
        )?;
        
        continue; // Skip to next tool
    }
}
```

**Handle abort during tool execution:**

When a tool is executing and abort is triggered, the tool execution should be cancelled. Update the tool execution loop (around line 249-301):

```rust
// Execute tools and send completion events
let mut aborted_tool_ids = Vec::new();

for tool_call in &tool_calls_with_unique_ids {
    // Check if cancelled before starting tool
    if cancel_token.is_cancelled() {
        aborted_tool_ids.push(tool_call.id.clone());
        
        let aborted_result = ToolResult {
            content: "Aborted".to_string(),
            is_error: Some(true),
            raw_response: None,
        };
        
        let _ = event_sender.send(ChatEvent::ToolCallCompleted {
            chat_id,
            tool_call_id: tool_call.id.clone(),
            result: aborted_result.content.clone(),
            is_error: true,
        });
        
        let tool_result_json = serde_json::json!({
            "toolCallId": tool_call.id,
            "name": tool_call.function.name,
            "result": aborted_result.content,
            "isError": true,
        })
        .to_string();
        
        let _ = manager.add_message_and_notify(
            chat_id,
            "tool",
            &tool_result_json,
            None,
            None,
            event_sender,
        )?;
        
        continue;
    }
    
    // Execute the tool
    let (tool_result, _) = execute_tool_call(manager, chat_id, tool_call, mcp_clients, event_sender).await;
    
    // ... rest of the existing code
}

// If any tools were aborted, transition to Aborted state
if !aborted_tool_ids.is_empty() {
    manager.abort_chat(chat_id, aborted_tool_ids).await;
    return Err(ChatError::Aborted);
}
```

**Handle pause during tool execution:**

After all tools in a batch complete, check if we should pause before continuing to the next iteration:

```rust
// After all tools complete, check if we should pause
if let Some(state_info) = manager.get_stream_state(chat_id).await {
    if state_info.is_paused {
        // All tool results have been inserted, exit loop
        return Err(ChatError::Paused);
    }
}
```

**Handle abort during AI call:**

When the AI call is cancelled, we need to discard the partial response and emit a `StreamAborted` event. Update the AI call section (around line 118-153):

```rust
let result = client
    .chat_stream_with_tools(
        api_model,
        &chat_messages,
        tools,
        cancel_token.clone(),
        move |chunk| {
            // ... existing chunk handling
        },
        raw_log_ref,
    )
    .await;

// Check if the call was aborted
match result {
    Ok(result) => {
        // Normal processing
    }
    Err(e) => {
        if let rhd_ai::client::AiError::Aborted { .. } = e {
            // Emit StreamAborted event
            let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
            
            // Transition to Aborted state
            manager.abort_chat(chat_id, Vec::new()).await;
            
            return Err(ChatError::Ai(e));
        }
        return Err(ChatError::Ai(e));
    }
}
```

**Update tool loop return type:**

The tool loop currently returns `Result<i64, ChatError>`. We need to handle the `Paused` case differently. Update the function signature and error handling:

```rust
pub async fn tool_loop<P: ProjectProvider>(
    // ... existing parameters
) -> Result<ToolLoopResult, ChatError> {
    // ... existing code
    
    // When pausing
    return Ok(ToolLoopResult::Paused { pending_tool_calls });
    
    // When completing normally
    return Ok(ToolLoopResult::Completed { message_id });
    
    // When aborting
    return Err(ChatError::Aborted);
}
```

Add new return type:

```rust
#[derive(Debug)]
pub enum ToolLoopResult {
    Completed { message_id: i64 },
    Paused { pending_tool_calls: Vec<PendingToolCall> },
}
```

### 2. `packages/rhd_chat/src/manager.rs`

**Add helper methods for tool loop integration:**

```rust
pub async fn get_stream_state(&self, chat_id: i64) -> Option<StreamStateInfo> {
    let active = self.active_streams.lock().await;
    active.get(&chat_id).map(|state| StreamStateInfo::from(state))
}

pub async fn is_aborted(&self, chat_id: i64) -> bool {
    let active = self.active_streams.lock().await;
    active.get(&chat_id).map(|s| s.is_aborted()).unwrap_or(false)
}

pub async fn set_pending_tool_calls(&self, chat_id: i64, pending_tool_calls: Vec<PendingToolCall>) {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Paused { cancel_token, pause_notify, phase, .. }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        let phase = phase.clone();
        active.insert(
            chat_id,
            StreamState::Paused {
                cancel_token,
                pause_notify,
                phase,
                pending_tool_calls,
            },
        );
    }
}

pub async fn set_execution_phase(&self, chat_id: i64, phase: ExecutionPhase) {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Running { cancel_token, pause_notify, .. }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        active.insert(
            chat_id,
            StreamState::Running {
                cancel_token,
                pause_notify,
                phase,
            },
        );
    }
}
```

**Update `resume_chat()` to return pending tool calls:**

```rust
pub async fn resume_chat(&self, chat_id: i64) -> Option<ResumeInfo> {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Paused { cancel_token, pause_notify, phase, pending_tool_calls }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        let pending_tool_calls = pending_tool_calls.clone();
        pause_notify.notify_one();
        active.insert(
            chat_id,
            StreamState::Running {
                cancel_token,
                pause_notify,
                phase: ExecutionPhase::AiCall,
            },
        );
        Some(ResumeInfo {
            pending_tool_calls,
            previous_phase: phase.clone(),
        })
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ResumeInfo {
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub previous_phase: ExecutionPhase,
}
```

### 3. `packages/rhd_chat/src/stream.rs`

**Handle abort during streaming:**

When abort is triggered during AI call, we need to discard the partial response and emit an event to remove the streaming message from the frontend.

Update the `send_message()` function to handle the `StreamAborted` event:

```rust
pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoaderRef,
) -> Result<i64, ChatError> {
    // ... existing code
    
    let result = tool_loop(
        manager,
        chat_id,
        model,
        &api_model,
        &client,
        &tools,
        &mcp_clients,
        &cancel_token,
        &event_sender,
        &mut iterations,
        &mut current_content,
        max_iterations,
        loggers,
        template_loader,
    )
    .await;
    
    match result {
        Ok(ToolLoopResult::Completed { message_id }) => {
            manager.unregister_stream(chat_id).await;
            Ok(message_id)
        }
        Ok(ToolLoopResult::Paused { pending_tool_calls }) => {
            // Stream is paused, don't unregister
            // The pending tool calls are stored in the state
            Ok(0) // Return dummy message_id
        }
        Err(ChatError::Aborted) => {
            // Emit StreamAborted event
            let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
            manager.unregister_stream(chat_id).await;
            Err(ChatError::Aborted)
        }
        Err(e) => {
            manager.unregister_stream(chat_id).await;
            Err(e)
        }
    }
}
```

### 4. `packages/rhd_chat/src/event.rs`

**Add `StreamAborted` event:**

```rust
pub enum ChatEvent {
    // ... existing events
    
    /// Stream was aborted by user
    StreamAborted { chat_id: i64 },
    
    // ... other events
}
```

### 5. `packages/rhd_chat/src/error.rs`

**Add `Paused` and `Aborted` error variants:**

```rust
pub enum ChatError {
    // ... existing errors
    
    /// Chat was paused by user
    Paused,
    
    /// Chat was aborted by user
    Aborted,
    
    // ... other errors
}
```

## Tests

### Unit Tests

Add to `packages/rhd_chat/src/tools/tool_loop.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_pause_during_ai_call_stores_pending_tools() {
        // Setup mock manager and client
        // Trigger AI call that returns tool calls
        // Pause before tool execution
        // Verify pending_tool_calls are stored
        // Verify tool loop exits with Paused result
    }
    
    #[tokio::test]
    async fn test_pause_during_tool_execution_completes_current_tools() {
        // Setup mock manager and client
        // Start tool execution
        // Pause during tool execution
        // Verify current tools complete
        // Verify tool results are inserted
        // Verify tool loop exits with Paused result
    }
    
    #[tokio::test]
    async fn test_abort_during_ai_call_discards_response() {
        // Setup mock manager and client
        // Start AI call
        // Abort during AI call
        // Verify partial response is discarded
        // Verify StreamAborted event is emitted
        // Verify no assistant message is saved
    }
    
    #[tokio::test]
    async fn test_abort_during_tool_execution_returns_errors() {
        // Setup mock manager and client
        // Start tool execution
        // Abort during tool execution
        // Verify non-finished tools return "Aborted" error
        // Verify finished tools keep their results
        // Verify all tool results are inserted
    }
}
```

### Integration Tests

Add to `packages/rhd_chat/tests/integration_tests.rs`:

```rust
#[tokio::test]
async fn test_tool_loop_pause_resume_cycle() {
    // Create chat
    // Start tool loop
    // Pause during AI call
    // Verify pending tool calls stored
    // Resume
    // Verify pending tool calls executed
    // Verify tool loop continues
}

#[tokio::test]
async fn test_tool_loop_abort_cleanup() {
    // Create chat
    // Start tool loop
    // Abort during tool execution
    // Verify aborted tools return errors
    // Verify StreamAborted event emitted
    // Verify state transitions to Aborted
}
```

## Implementation Notes

1. **Pause check placement**: The pause check is moved from the start of the iteration to two critical points:
   - After AI call completes (before tool execution) - to handle pause during AI call
   - After all tools complete (before next iteration) - to handle pause during tool execution

2. **Abort during AI call**: When the AI call is cancelled, we:
   - Catch the `AiError::Aborted` error
   - Emit `StreamAborted` event to frontend
   - Transition to `Aborted` state
   - Do NOT save the assistant message
   - Return error to exit tool loop

3. **Abort during tool execution**: When abort is triggered during tool execution:
   - Check `cancel_token.is_cancelled()` before each tool
   - Non-finished tools return "Aborted" error result
   - Finished tools keep their results
   - All tool results (finished + aborted) are inserted into chat messages
   - Transition to `Aborted` state after all tools complete

4. **Pause during AI call**: When pause is triggered during AI call:
   - Wait for AI to complete naturally (don't cancel)
   - Store tool calls in `pending_tool_calls` without executing
   - Exit tool loop with `ToolLoopResult::Paused`
   - State remains `Paused` with pending tool calls

5. **Pause during tool execution**: When pause is triggered during tool execution:
   - Wait for current tool calls to complete
   - Insert tool results into chat messages
   - Check pause state after all tools complete
   - Exit tool loop with `ToolLoopResult::Paused`
   - Do NOT continue to next iteration

6. **Execution phase tracking**: The `ExecutionPhase` is updated as the tool loop progresses:
   - Set to `AiCall` before AI call
   - Set to `ToolExecution` before tool execution
   - This helps determine the correct pause/abort behavior

7. **Error handling**: New error variants `Paused` and `Aborted` are added to distinguish between normal errors and user-initiated pause/abort operations.

## Dependencies

- This phase depends on: Phase 1 (Backend State Machine)
- This phase must be completed before: Phase 3 (Message Queue)
- Phase 4 (Frontend Integration) can start after this phase
