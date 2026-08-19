use std::sync::Arc;

mod action;
mod error;
mod event;
mod input;
mod listener;
mod state;

#[cfg(test)]
mod tests;

pub use action::ToolLoopAction;
pub use error::FsmError;
pub use event::ToolLoopFsmEvent;
pub use input::ToolLoopInput;
pub use listener::{ToolLoopListenerCallback, ToolLoopListenerId, ToolLoopListenerManager};
pub use state::{ChatMessage, PendingToolCall, State, ToolCall, ToolDefinition, ToolResult};


/// Finite State Machine for managing the tool loop
pub struct ToolLoopFsm {
    state: State,
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDefinition>,
    tool_call_id_counter: u64,
    message_id_counter: i64,
    listener_manager: ToolLoopListenerManager,
}

impl ToolLoopFsm {
    /// Create a new FSM in Idle state
    pub fn new() -> Self {
        Self::with_message_id_counter(1_000_000)
    }

    /// Create a new FSM with a specific initial message ID counter
    pub fn with_message_id_counter(initial_message_id: i64) -> Self {
        Self {
            state: State::Idle,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_call_id_counter: 0,
            message_id_counter: initial_message_id,
            listener_manager: ToolLoopListenerManager::new(),
        }
    }

    /// Register a listener for FSM events
    pub fn add_listener(&mut self, callback: ToolLoopListenerCallback) -> ToolLoopListenerId {
        self.listener_manager.add_listener(callback)
    }

    /// Remove a listener by its ID
    pub fn remove_listener(&mut self, id: ToolLoopListenerId) -> bool {
        self.listener_manager.remove_listener(id)
    }

    /// Emit an event to all registered listeners
    fn emit_event(&self, event: ToolLoopFsmEvent) {
        self.listener_manager.emit(event);
    }

    /// Get current state
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Check if FSM is in terminal state
    pub fn is_finished(&self) -> bool {
        self.state.is_terminal()
    }

    /// Get current messages
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get available tools
    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    /// Main entry point - process inputs and return actions
    pub fn run(&mut self, inputs: &mut Vec<ToolLoopInput>) -> Result<Vec<ToolLoopAction>, FsmError> {
        let mut actions = Vec::new();

        tracing::debug!(
            fsm = "ToolLoopFsm",
            state = %self.state,
            inputs_count = inputs.len(),
            "ToolLoopFsm::run started"
        );

        for input in inputs.drain(..) {
            tracing::debug!(
                fsm = "ToolLoopFsm",
                state_before = %self.state,
                input = %input,
                "Processing input"
            );

            let input_actions = self.process_input(input)?;
            
            for action in &input_actions {
                tracing::debug!(
                    fsm = "ToolLoopFsm",
                    state_after = %self.state,
                    action = %action,
                    "Emitted action"
                );
            }
            
            actions.extend(input_actions);
        }

        tracing::debug!(
            fsm = "ToolLoopFsm",
            final_state = %self.state,
            actions_count = actions.len(),
            "ToolLoopFsm::run completed"
        );

        Ok(actions)
    }

    /// Process a single input and return resulting actions
    fn process_input(&mut self, input: ToolLoopInput) -> Result<Vec<ToolLoopAction>, FsmError> {
        // Check if FSM is finished
        if self.is_finished() {
            return Err(FsmError::FsmFinished {
                state: self.state.name().to_string(),
            });
        }

        match (&self.state, input) {
            // Idle state handlers
            (State::Idle, ToolLoopInput::Run) => self.handle_run(),
            (State::Idle, ToolLoopInput::InsertMessage { message }) => {
                self.handle_insert_message(message)
            }
            (State::Idle, ToolLoopInput::RemoveMessage { message_id }) => {
                self.handle_remove_message(message_id)
            }
            (State::Idle, ToolLoopInput::ReplaceMessage { message_id, new_message }) => {
                self.handle_replace_message(message_id, new_message)
            }
            (State::Idle, ToolLoopInput::ReplaceAllMessages { messages }) => {
                self.handle_replace_all_messages(messages)
            }
            (State::Idle, ToolLoopInput::RequestToolCallId) => {
                self.handle_request_tool_call_id()
            }
            (State::Idle, ToolLoopInput::AddTool { tool }) => self.handle_add_tool(tool),
            (State::Idle, ToolLoopInput::RemoveTool { tool_name }) => {
                self.handle_remove_tool(tool_name)
            }
            (State::Idle, ToolLoopInput::Abort) => self.handle_abort(),

            // AwaitingAiResponse state handlers
            (State::AwaitingAiResponse { .. }, ToolLoopInput::ProvideAiResponse {
                content,
                thinking_content,
                tool_calls,
                finish_reason,
            }) => self.handle_provide_ai_response(content, thinking_content, tool_calls, finish_reason),
            (State::AwaitingAiResponse { .. }, ToolLoopInput::Pause) => self.handle_pause(),
            (State::AwaitingAiResponse { .. }, ToolLoopInput::Abort) => self.handle_abort(),

            // AwaitingToolResults state handlers
            (State::AwaitingToolResults { .. }, ToolLoopInput::ProvideToolResult { tool_call_id, result }) => {
                self.handle_provide_tool_result(tool_call_id, result)
            }
            (State::AwaitingToolResults { .. }, ToolLoopInput::Pause) => self.handle_pause(),
            (State::AwaitingToolResults { .. }, ToolLoopInput::Abort) => self.handle_abort(),

            // Paused state handlers
            (State::Paused { .. }, ToolLoopInput::Resume) => self.handle_resume(),
            (State::Paused { .. }, ToolLoopInput::Abort) => self.handle_abort(),

            // Message/tool management can happen in non-terminal states
            (state, ToolLoopInput::InsertMessage { message }) if !state.is_terminal() => {
                self.handle_insert_message(message)
            }
            (state, ToolLoopInput::RemoveMessage { message_id }) if !state.is_terminal() => {
                self.handle_remove_message(message_id)
            }
            (state, ToolLoopInput::ReplaceMessage { message_id, new_message }) if !state.is_terminal() => {
                self.handle_replace_message(message_id, new_message)
            }
            (state, ToolLoopInput::ReplaceAllMessages { messages }) if !state.is_terminal() => {
                self.handle_replace_all_messages(messages)
            }
            (state, ToolLoopInput::RequestToolCallId) if !state.is_terminal() => {
                self.handle_request_tool_call_id()
            }
            (state, ToolLoopInput::AddTool { tool }) if !state.is_terminal() => {
                self.handle_add_tool(tool)
            }
            (state, ToolLoopInput::RemoveTool { tool_name }) if !state.is_terminal() => {
                self.handle_remove_tool(tool_name)
            }

            // Invalid transitions
            (state, input) => Err(FsmError::UnexpectedInput {
                state: state.name().to_string(),
                input: input.name().to_string(),
            }),
        }
    }

    /// Handle Run input - start the tool loop
    fn handle_run(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();
        let sent_messages = self.messages.clone();
        let tools = self.tools.clone();

        self.state = State::AwaitingAiResponse { sent_messages };

        self.emit_event(ToolLoopFsmEvent::StateChanged {
            from: old_state,
            to: self.state.clone(),
        });

        Ok(vec![ToolLoopAction::SendToAi {
            messages: self.messages.clone(),
            tools,
        }])
    }

    /// Handle InsertMessage input
    fn handle_insert_message(&mut self, message: ChatMessage) -> Result<Vec<ToolLoopAction>, FsmError> {
        self.messages.push(message.clone());
        self.emit_event(ToolLoopFsmEvent::MessageInserted { message });
        Ok(Vec::new())
    }

    /// Handle RemoveMessage input
    fn handle_remove_message(&mut self, message_id: i64) -> Result<Vec<ToolLoopAction>, FsmError> {
        let initial_len = self.messages.len();
        self.messages.retain(|m| m.id != message_id);

        if self.messages.len() == initial_len {
            return Err(FsmError::MessageNotFound { message_id });
        }

        self.emit_event(ToolLoopFsmEvent::MessageRemoved { message_id });
        Ok(Vec::new())
    }

    /// Handle ReplaceMessage input
    fn handle_replace_message(&mut self, message_id: i64, new_message: ChatMessage) -> Result<Vec<ToolLoopAction>, FsmError> {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            *msg = new_message.clone();
            self.emit_event(ToolLoopFsmEvent::MessageReplaced { message_id, new_message });
            Ok(Vec::new())
        } else {
            Err(FsmError::MessageNotFound { message_id })
        }
    }

    /// Handle ReplaceAllMessages input
    fn handle_replace_all_messages(&mut self, messages: Vec<ChatMessage>) -> Result<Vec<ToolLoopAction>, FsmError> {
        self.messages = messages.clone();
        self.emit_event(ToolLoopFsmEvent::AllMessagesReplaced { messages });
        Ok(Vec::new())
    }

    /// Handle RequestToolCallId input
    fn handle_request_tool_call_id(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
        let tool_call_id = format!("call_{}", self.tool_call_id_counter);
        self.tool_call_id_counter += 1;
        self.emit_event(ToolLoopFsmEvent::ToolCallIdGenerated { tool_call_id: tool_call_id.clone() });
        Ok(vec![ToolLoopAction::GenerateToolCallId { tool_call_id }])
    }

    /// Handle AddTool input
    fn handle_add_tool(&mut self, tool: ToolDefinition) -> Result<Vec<ToolLoopAction>, FsmError> {
        self.tools.push(tool);
        Ok(Vec::new())
    }

    /// Handle RemoveTool input
    fn handle_remove_tool(&mut self, tool_name: String) -> Result<Vec<ToolLoopAction>, FsmError> {
        let initial_len = self.tools.len();
        self.tools.retain(|t| t.name != tool_name);

        if self.tools.len() == initial_len {
            return Err(FsmError::ToolNotFound { tool_name });
        }

        Ok(Vec::new())
    }

    /// Handle ProvideAiResponse input
    fn handle_provide_ai_response(
        &mut self,
        content: Option<String>,
        thinking_content: Option<String>,
        tool_calls: Vec<ToolCall>,
        _finish_reason: String,
    ) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();

        // Extract sent_messages from current state
        let State::AwaitingAiResponse { sent_messages: _ } = &self.state else {
            return Err(FsmError::InvalidTransition {
                from_state: self.state.name().to_string(),
                input: "ProvideAiResponse".to_string(),
            });
        };

        // Emit AI response event
        self.emit_event(ToolLoopFsmEvent::AiResponseReceived {
            content: content.clone(),
            thinking_content: thinking_content.clone(),
            tool_calls: tool_calls.clone(),
        });

        if tool_calls.is_empty() {
            // Final response - complete the loop
            let message_id = self.generate_message_id();
            let final_message = ChatMessage {
                id: message_id,
                role: "assistant".to_string(),
                content: content.unwrap_or_default(),
                thinking_content,
                tool_calls: None,
            };

            self.messages.push(final_message.clone());
            self.state = State::Completed { message_id };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });
            self.emit_event(ToolLoopFsmEvent::MessageInserted { message: final_message.clone() });

            Ok(vec![ToolLoopAction::Completed { message: final_message }])
        } else {
            // Tool calls received - transition to AwaitingToolResults
            let mut actions = Vec::new();
            let mut pending_tool_calls = Vec::new();

            // Create assistant message with tool calls
            let message_id = self.generate_message_id();
            let assistant_message = ChatMessage {
                id: message_id,
                role: "assistant".to_string(),
                content: content.unwrap_or_default(),
                thinking_content,
                tool_calls: Some(tool_calls.clone()),
            };
            self.messages.push(assistant_message.clone());
            self.emit_event(ToolLoopFsmEvent::MessageInserted { message: assistant_message });

            // Create pending tool calls and emit ExecuteToolCall actions
            for tool_call in tool_calls {
                pending_tool_calls.push(PendingToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                });

                // Emit ToolCallRequested event with propagate flag
                let propagate = Arc::new(std::sync::atomic::AtomicBool::new(true));
                self.emit_event(ToolLoopFsmEvent::ToolCallRequested {
                    tool_call: tool_call.clone(),
                    propagate: propagate.clone(),
                });

                // Only add ExecuteToolCall action if propagate is still true
                if propagate.load(std::sync::atomic::Ordering::SeqCst) {
                    actions.push(ToolLoopAction::ExecuteToolCall { tool_call });
                }
            }

            self.state = State::AwaitingToolResults {
                pending_tool_calls,
                collected_results: Vec::new(),
            };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });

            Ok(actions)
        }
    }

    /// Handle ProvideToolResult input
    fn handle_provide_tool_result(
        &mut self,
        tool_call_id: String,
        result: ToolResult,
    ) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();

        // Extract needed data from state first
        let (tool_name, pending_count) = match &self.state {
            State::AwaitingToolResults { pending_tool_calls, collected_results } => {
                let tool_name = pending_tool_calls
                    .iter()
                    .find(|tc| tc.id == tool_call_id)
                    .map(|tc| tc.name.clone())
                    .ok_or_else(|| FsmError::UnknownToolCall { tool_call_id: tool_call_id.clone() })?;
                (tool_name, (pending_tool_calls.len(), collected_results.len()))
            }
            _ => {
                return Err(FsmError::InvalidTransition {
                    from_state: self.state.name().to_string(),
                    input: "ProvideToolResult".to_string(),
                });
            }
        };

        // Generate message ID and create tool message
        let message_id = self.generate_message_id();
        let tool_message = ChatMessage {
            id: message_id,
            role: "tool".to_string(),
            content: serde_json::to_string(&serde_json::json!({
                "toolCallId": result.tool_call_id,
                "name": tool_name,
                "result": result.content,
                "isError": result.is_error,
            })).unwrap_or_default(),
            thinking_content: None,
            tool_calls: None,
        };
        self.messages.push(tool_message.clone());
        self.emit_event(ToolLoopFsmEvent::MessageInserted { message: tool_message });

        // Emit ToolCallExecuted event
        let tool_call = ToolCall {
            id: result.tool_call_id.clone(),
            name: tool_name,
            arguments: String::new(), // Arguments not available at this point
        };
        self.emit_event(ToolLoopFsmEvent::ToolCallExecuted {
            tool_call,
            result: result.clone(),
        });

        // Update state
        let (pending_count, _collected_count) = pending_count;
        let collected_results = match &mut self.state {
            State::AwaitingToolResults { collected_results, .. } => collected_results,
            _ => panic!("State changed unexpectedly"),
        };
        collected_results.push(result);

        // Check if all tool calls are resolved
        if collected_results.len() == pending_count {
            let sent_messages = self.messages.clone();
            let tools = self.tools.clone();

            self.state = State::AwaitingAiResponse { sent_messages };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });

            Ok(vec![ToolLoopAction::SendToAi {
                messages: self.messages.clone(),
                tools,
            }])
        } else {
            Ok(Vec::new())
        }
    }

    /// Handle Pause input
    fn handle_pause(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();

        let (pending_tool_calls, collected_results) = match &self.state {
            State::AwaitingAiResponse { .. } => (Vec::new(), Vec::new()),
            State::AwaitingToolResults {
                pending_tool_calls,
                collected_results,
            } => (pending_tool_calls.clone(), collected_results.clone()),
            _ => {
                return Err(FsmError::InvalidTransition {
                    from_state: self.state.name().to_string(),
                    input: "Pause".to_string(),
                });
            }
        };

        self.state = State::Paused {
            pending_tool_calls,
            collected_results,
        };

        self.emit_event(ToolLoopFsmEvent::StateChanged {
            from: old_state,
            to: self.state.clone(),
        });

        Ok(vec![ToolLoopAction::Paused])
    }

    /// Handle Resume input
    fn handle_resume(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();

        let State::Paused {
            pending_tool_calls,
            collected_results,
        } = &self.state else {
            return Err(FsmError::InvalidTransition {
                from_state: self.state.name().to_string(),
                input: "Resume".to_string(),
            });
        };

        let pending_tool_calls = pending_tool_calls.clone();
        let collected_results = collected_results.clone();

        if pending_tool_calls.is_empty() {
            // Was paused during AI call - resume by sending to AI
            let sent_messages = self.messages.clone();
            let tools = self.tools.clone();

            self.state = State::AwaitingAiResponse { sent_messages };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });

            Ok(vec![ToolLoopAction::SendToAi {
                messages: self.messages.clone(),
                tools,
            }])
        } else if collected_results.len() == pending_tool_calls.len() {
            // All tool results were collected before pause - send to AI
            let sent_messages = self.messages.clone();
            let tools = self.tools.clone();

            self.state = State::AwaitingAiResponse { sent_messages };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });

            Ok(vec![ToolLoopAction::SendToAi {
                messages: self.messages.clone(),
                tools,
            }])
        } else {
            // Still have pending tool calls - resume awaiting results
            self.state = State::AwaitingToolResults {
                pending_tool_calls,
                collected_results,
            };

            self.emit_event(ToolLoopFsmEvent::StateChanged {
                from: old_state,
                to: self.state.clone(),
            });

            Ok(Vec::new())
        }
    }

    /// Handle Abort input
    fn handle_abort(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
        let old_state = self.state.clone();
        self.state = State::Aborted;
        self.emit_event(ToolLoopFsmEvent::StateChanged {
            from: old_state,
            to: self.state.clone(),
        });
        Ok(vec![ToolLoopAction::Aborted])
    }

    /// Generate a unique message ID using the instance counter
    fn generate_message_id(&mut self) -> i64 {
        let id = self.message_id_counter;
        self.message_id_counter += 1;
        id
    }
}

impl Default for ToolLoopFsm {
    fn default() -> Self {
        Self::new()
    }
}
