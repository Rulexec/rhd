# Phase 1: FSM Listener System

## Overview

This phase adds a listener system and event enum to the existing `ToolLoopFsm` to enable external synchronization and extensibility. The listener system allows external components (DB sync, helper FSMs, logging) to react to FSM state transitions and message mutations.

**Scope:**
- Add `ToolLoopFsmEvent` enum with all event variants
- Add listener management methods (`add_listener`, `remove_listener`)
- Add `message_id_counter` field to FSM for deterministic ID generation
- Emit events during state transitions and message mutations
- Add unit tests for the listener system

**Out of Scope:**
- Async wrapper implementation (Phase 2)
- DB synchronization (Phase 3)
- Helper FSMs (Phase 6)

## Files to Modify

### 1. `packages/rhd_fsm/src/tool_loop_fsm/event.rs` (NEW FILE)

**Purpose:** Define the `ToolLoopFsmEvent` enum that represents all events emitted by the FSM.

**Complete Implementation:**

```rust
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::state::{ChatMessage, State, ToolCall, ToolResult};

/// Events emitted by the ToolLoopFsm during state transitions and message mutations
#[derive(Debug, Clone)]
pub enum ToolLoopFsmEvent {
    /// A new message was inserted into the conversation
    MessageInserted {
        message: ChatMessage,
    },

    /// A message was removed from the conversation
    MessageRemoved {
        message_id: i64,
    },

    /// A message was replaced with a new one
    MessageReplaced {
        message_id: i64,
        new_message: ChatMessage,
    },

    /// All messages were replaced (bulk update)
    AllMessagesReplaced {
        messages: Vec<ChatMessage>,
    },

    /// A tool call was requested and is about to be executed
    /// Listeners can set `propagate` to false to intercept and handle the tool call themselves
    ToolCallRequested {
        tool_call: ToolCall,
        propagate: Arc<AtomicBool>,
    },

    /// A tool call was executed and produced a result
    ToolCallExecuted {
        tool_call: ToolCall,
        result: ToolResult,
    },

    /// AI response was received
    AiResponseReceived {
        content: Option<String>,
        thinking_content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },

    /// A new tool call ID was generated
    ToolCallIdGenerated {
        tool_call_id: String,
    },

    /// FSM state changed
    StateChanged {
        from: State,
        to: State,
    },
}

impl ToolLoopFsmEvent {
    /// Returns the name of the event as a string
    pub fn name(&self) -> &'static str {
        match self {
            ToolLoopFsmEvent::MessageInserted { .. } => "MessageInserted",
            ToolLoopFsmEvent::MessageRemoved { .. } => "MessageRemoved",
            ToolLoopFsmEvent::MessageReplaced { .. } => "MessageReplaced",
            ToolLoopFsmEvent::AllMessagesReplaced { .. } => "AllMessagesReplaced",
            ToolLoopFsmEvent::ToolCallRequested { .. } => "ToolCallRequested",
            ToolLoopFsmEvent::ToolCallExecuted { .. } => "ToolCallExecuted",
            ToolLoopFsmEvent::AiResponseReceived { .. } => "AiResponseReceived",
            ToolLoopFsmEvent::ToolCallIdGenerated { .. } => "ToolCallIdGenerated",
            ToolLoopFsmEvent::StateChanged { .. } => "StateChanged",
        }
    }
}
```

### 2. `packages/rhd_fsm/src/tool_loop_fsm/listener.rs` (NEW FILE)

**Purpose:** Define listener types and management structures.

**Complete Implementation:**

```rust
use std::sync::Arc;

use super::event::ToolLoopFsmEvent;

/// Unique identifier for a registered listener
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolLoopListenerId(pub usize);

/// Type alias for listener callback function
pub type ToolLoopListenerCallback = Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync>;

/// Manages a collection of listeners for FSM events
pub struct ToolLoopListenerManager {
    listeners: Vec<(ToolLoopListenerId, ToolLoopListenerCallback)>,
    next_id: usize,
}

impl ToolLoopListenerManager {
    /// Create a new listener manager
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
            next_id: 0,
        }
    }

    /// Register a new listener and return its ID
    pub fn add_listener(&mut self, callback: ToolLoopListenerCallback) -> ToolLoopListenerId {
        let id = ToolLoopListenerId(self.next_id);
        self.next_id += 1;
        self.listeners.push((id, callback));
        id
    }

    /// Remove a listener by its ID
    pub fn remove_listener(&mut self, id: ToolLoopListenerId) -> bool {
        let initial_len = self.listeners.len();
        self.listeners.retain(|(listener_id, _)| *listener_id != id);
        self.listeners.len() < initial_len
    }

    /// Emit an event to all registered listeners
    pub fn emit(&self, event: ToolLoopFsmEvent) {
        for (_, callback) in &self.listeners {
            callback(event.clone());
        }
    }

    /// Get the number of registered listeners
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

impl Default for ToolLoopListenerManager {
    fn default() -> Self {
        Self::new()
    }
}
```

### 3. `packages/rhd_fsm/src/tool_loop_fsm/mod.rs`

**Modifications:**

1. Add module declarations for new files:
```rust
mod event;
mod listener;
```

2. Add public exports:
```rust
pub use event::ToolLoopFsmEvent;
pub use listener::{ToolLoopListenerCallback, ToolLoopListenerId, ToolLoopListenerManager};
```

3. Modify `ToolLoopFsm` struct to include listener manager and message ID counter:
```rust
pub struct ToolLoopFsm {
    state: State,
    messages: Vec<ChatMessage>,
    tools: Vec<ToolDefinition>,
    tool_call_id_counter: u64,
    message_id_counter: i64,  // NEW: For deterministic message ID generation
    listener_manager: ToolLoopListenerManager,  // NEW: For event emission
}
```

4. Modify `new()` constructor to accept initial message ID counter:
```rust
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
    
    // ... existing methods ...
}
```

5. Add listener management methods:
```rust
impl ToolLoopFsm {
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
    
    // ... rest of existing methods ...
}
```

6. Modify `generate_message_id()` to use instance counter and emit event:
```rust
impl ToolLoopFsm {
    /// Generate a unique message ID using the instance counter
    fn generate_message_id(&mut self) -> i64 {
        let id = self.message_id_counter;
        self.message_id_counter += 1;
        id
    }
}
```

7. Modify state transition handlers to emit events. Update each handler:

**`handle_insert_message`:**
```rust
fn handle_insert_message(&mut self, message: ChatMessage) -> Result<Vec<ToolLoopAction>, FsmError> {
    self.messages.push(message.clone());
    self.emit_event(ToolLoopFsmEvent::MessageInserted { message });
    Ok(Vec::new())
}
```

**`handle_remove_message`:**
```rust
fn handle_remove_message(&mut self, message_id: i64) -> Result<Vec<ToolLoopAction>, FsmError> {
    let initial_len = self.messages.len();
    self.messages.retain(|m| m.id != message_id);

    if self.messages.len() == initial_len {
        return Err(FsmError::MessageNotFound { message_id });
    }

    self.emit_event(ToolLoopFsmEvent::MessageRemoved { message_id });
    Ok(Vec::new())
}
```

**`handle_replace_message`:**
```rust
fn handle_replace_message(&mut self, message_id: i64, new_message: ChatMessage) -> Result<Vec<ToolLoopAction>, FsmError> {
    if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
        *msg = new_message.clone();
        self.emit_event(ToolLoopFsmEvent::MessageReplaced { message_id, new_message });
        Ok(Vec::new())
    } else {
        Err(FsmError::MessageNotFound { message_id })
    }
}
```

**`handle_replace_all_messages`:**
```rust
fn handle_replace_all_messages(&mut self, messages: Vec<ChatMessage>) -> Result<Vec<ToolLoopAction>, FsmError> {
    self.messages = messages.clone();
    self.emit_event(ToolLoopFsmEvent::AllMessagesReplaced { messages });
    Ok(Vec::new())
}
```

**`handle_request_tool_call_id`:**
```rust
fn handle_request_tool_call_id(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
    let tool_call_id = format!("call_{}", self.tool_call_id_counter);
    self.tool_call_id_counter += 1;
    self.emit_event(ToolLoopFsmEvent::ToolCallIdGenerated { tool_call_id: tool_call_id.clone() });
    Ok(vec![ToolLoopAction::GenerateToolCallId { tool_call_id }])
}
```

**`handle_run` - emit StateChanged:**
```rust
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
```

**`handle_provide_ai_response` - emit events:**
```rust
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
```

**`handle_provide_tool_result` - emit events:**
```rust
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
```

**`handle_pause` - emit StateChanged:**
```rust
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
```

**`handle_resume` - emit StateChanged:**
```rust
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
```

**`handle_abort` - emit StateChanged:**
```rust
fn handle_abort(&mut self) -> Result<Vec<ToolLoopAction>, FsmError> {
    let old_state = self.state.clone();
    self.state = State::Aborted;
    self.emit_event(ToolLoopFsmEvent::StateChanged {
        from: old_state,
        to: self.state.clone(),
    });
    Ok(vec![ToolLoopAction::Aborted])
}
```

### 4. `packages/rhd_fsm/src/tool_loop_fsm/tests.rs`

**Add new test module for listener system:**

```rust
mod listener_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_add_and_remove_listener() {
        let mut fsm = ToolLoopFsm::new();
        
        let event_log = Arc::new(Mutex::new(Vec::new()));
        let event_log_clone = event_log.clone();
        
        let listener_id = fsm.add_listener(Arc::new(move |event| {
            event_log_clone.lock().unwrap().push(event.name().to_string());
        }));
        
        assert_eq!(fsm.listener_manager.listener_count(), 1);
        
        let removed = fsm.remove_listener(listener_id);
        assert!(removed);
        assert_eq!(fsm.listener_manager.listener_count(), 0);
    }

    #[test]
    fn test_listener_receives_message_events() {
        let mut fsm = ToolLoopFsm::new();
        
        let event_log = Arc::new(Mutex::new(Vec::new()));
        let event_log_clone = event_log.clone();
        
        fsm.add_listener(Arc::new(move |event| {
            event_log_clone.lock().unwrap().push(event.name().to_string());
        }));
        
        let message = ChatMessage {
            id: 1,
            role: "user".to_string(),
            content: "Hello".to_string(),
            thinking_content: None,
            tool_calls: None,
        };
        
        let mut inputs = vec![ToolLoopInput::InsertMessage { message }];
        let _ = fsm.run(&mut inputs).unwrap();
        
        let events = event_log.lock().unwrap();
        assert!(events.contains(&"MessageInserted".to_string()));
    }

    #[test]
    fn test_listener_receives_state_changed_events() {
        let mut fsm = ToolLoopFsm::new();
        
        let event_log = Arc::new(Mutex::new(Vec::new()));
        let event_log_clone = event_log.clone();
        
        fsm.add_listener(Arc::new(move |event| {
            event_log_clone.lock().unwrap().push(event.name().to_string());
        }));
        
        let mut inputs = vec![ToolLoopInput::Run];
        let _ = fsm.run(&mut inputs).unwrap();
        
        let events = event_log.lock().unwrap();
        assert!(events.contains(&"StateChanged".to_string()));
    }

    #[test]
    fn test_message_id_counter() {
        let mut fsm = ToolLoopFsm::with_message_id_counter(100);
        
        let id1 = fsm.generate_message_id();
        let id2 = fsm.generate_message_id();
        
        assert_eq!(id1, 100);
        assert_eq!(id2, 101);
    }

    #[test]
    fn test_tool_call_id_generated_event() {
        let mut fsm = ToolLoopFsm::new();
        
        let event_log = Arc::new(Mutex::new(Vec::new()));
        let event_log_clone = event_log.clone();
        
        fsm.add_listener(Arc::new(move |event| {
            event_log_clone.lock().unwrap().push(event.name().to_string());
        }));
        
        let mut inputs = vec![ToolLoopInput::RequestToolCallId];
        let _ = fsm.run(&mut inputs).unwrap();
        
        let events = event_log.lock().unwrap();
        assert!(events.contains(&"ToolCallIdGenerated".to_string()));
    }
}
```

## Tests

### Unit Tests

The tests are included in the `tests.rs` file modifications above. They cover:

1. **`test_add_and_remove_listener`**: Verifies listener registration and removal
2. **`test_listener_receives_message_events`**: Verifies MessageInserted event emission
3. **`test_listener_receives_state_changed_events`**: Verifies StateChanged event emission
4. **`test_message_id_counter`**: Verifies deterministic message ID generation
5. **`test_tool_call_id_generated_event`**: Verifies ToolCallIdGenerated event emission

### Running Tests

```bash
cd packages/rhd_fsm
cargo test tool_loop_fsm::tests::listener_tests
```

## Implementation Notes

1. **Event Emission Timing**: Events are emitted AFTER the state change or message mutation has occurred. This ensures listeners always see consistent state.

2. **Propagate Flag**: The `ToolCallRequested` event includes an `Arc<AtomicBool>` that listeners can set to `false` to prevent the tool call from being executed. This enables helper FSMs to intercept built-in tools.

3. **Message ID Counter**: The FSM now uses an instance-level counter instead of a global static counter. This allows for deterministic testing and proper ID management when the FSM is the source of truth.

4. **Listener Callback Type**: Listeners use `Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync>` for simplicity and thread safety. The callback receives a cloned event.

5. **State Cloning**: The `State` enum must implement `Clone` for the `StateChanged` event. This is already the case in the existing implementation.

## Dependencies

- This phase has no dependencies on other phases
- This phase must be completed before Phase 2 (Async Wrapper) and Phase 3 (DB Sync Listener)

## Success Criteria

- [ ] FSM can register and unregister listeners
- [ ] Listeners receive events for all state transitions
- [ ] Listeners receive events for all message mutations
- [ ] Message ID counter works correctly and is deterministic
- [ ] ToolCallRequested event includes propagate flag
- [ ] Existing unit tests still pass
- [ ] New unit tests for listener system pass
- [ ] Code compiles without warnings
