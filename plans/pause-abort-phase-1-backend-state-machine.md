# Phase 1: Backend State Machine

## Overview

This phase redesigns the `StreamState` enum to properly distinguish between running, paused, and aborted states, and tracks execution phase (AI call vs tool execution). This is the foundation for all pause/abort/resume functionality.

**Scope:**
- Add `Aborted` variant to `StreamState`
- Add `ExecutionPhase` enum to track AI call vs tool execution
- Add fields to store pending tool calls and aborted tool IDs
- Update `ChatManager` methods to use the new state machine

**Out of scope:**
- Tool loop integration (Phase 2)
- Message queue (Phase 3)
- Frontend changes (Phase 4)

## Files to Modify

### 1. `packages/rhd_chat/src/state.rs`

**Add `ExecutionPhase` enum:**

```rust
/// Tracks which phase of the tool loop is currently executing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// AI is being called (streaming response)
    AiCall,
    /// Tool calls are being executed
    ToolExecution,
}
```

**Add `PendingToolCall` struct:**

```rust
/// Represents a tool call that was emitted by AI but not yet executed
#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
```

**Redesign `StreamState` enum:**

```rust
#[derive(Debug)]
pub enum StreamState {
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
        pending_tool_calls: Vec<PendingToolCall>,
    },
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
    },
}
```

**Update `StreamState` methods:**

```rust
impl StreamState {
    pub fn cancel_token(&self) -> &CancellationToken {
        match self {
            StreamState::Running { cancel_token, .. } => cancel_token,
            StreamState::Paused { cancel_token, .. } => cancel_token,
            StreamState::Aborted { cancel_token, .. } => cancel_token,
        }
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self, StreamState::Aborted { .. })
    }

    pub fn phase(&self) -> Option<&ExecutionPhase> {
        match self {
            StreamState::Running { phase, .. } => Some(phase),
            StreamState::Paused { phase, .. } => Some(phase),
            StreamState::Aborted { .. } => None,
        }
    }

    pub fn pending_tool_calls(&self) -> Option<&Vec<PendingToolCall>> {
        match self {
            StreamState::Paused { pending_tool_calls, .. } => Some(pending_tool_calls),
            _ => None,
        }
    }
}
```

### 2. `packages/rhd_chat/src/manager.rs`

**Update `register_stream()` to initialize with `ExecutionPhase::AiCall`:**

```rust
pub(crate) async fn register_stream(
    &self,
    chat_id: i64,
) -> (CancellationToken, Arc<Notify>) {
    let cancel_token = CancellationToken::new();
    let pause_notify = Arc::new(Notify::new());
    let mut active = self.active_streams.lock().await;
    if let Some(existing) = active.get(&chat_id) {
        existing.cancel_token().cancel();
    }
    active.insert(
        chat_id,
        StreamState::Running {
            cancel_token: cancel_token.clone(),
            pause_notify: pause_notify.clone(),
            phase: ExecutionPhase::AiCall,
        },
    );
    (cancel_token, pause_notify)
}
```

**Update `pause_chat()` to preserve execution phase and pending tool calls:**

```rust
pub async fn pause_chat(
    &self,
    chat_id: i64,
    pending_tool_calls: Vec<PendingToolCall>,
) -> bool {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Running { cancel_token, pause_notify, phase }) = active.get(&chat_id) {
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
        true
    } else {
        false
    }
}
```

**Update `abort_chat()` to transition to `Aborted` state:**

```rust
pub async fn abort_chat(&self, chat_id: i64, aborted_tool_ids: Vec<String>) -> bool {
    let mut active = self.active_streams.lock().await;
    if let Some(state) = active.get(&chat_id) {
        let cancel_token = state.cancel_token().clone();
        cancel_token.cancel();
        active.insert(
            chat_id,
            StreamState::Aborted {
                cancel_token,
                aborted_tool_ids,
            },
        );
        true
    } else {
        false
    }
}
```

**Update `resume_chat()` to transition from `Paused` to `Running`:**

```rust
pub async fn resume_chat(&self, chat_id: i64) -> Option<(Vec<PendingToolCall>, ExecutionPhase)> {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Paused { cancel_token, pause_notify, phase, pending_tool_calls }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        let phase = phase.clone();
        let pending_tool_calls = pending_tool_calls.clone();
        pause_notify.notify_one();
        active.insert(
            chat_id,
            StreamState::Running {
                cancel_token,
                pause_notify,
                phase: ExecutionPhase::AiCall, // Reset to AiCall on resume
            },
        );
        Some((pending_tool_calls, phase))
    } else {
        None
    }
}
```

**Add helper methods:**

```rust
pub async fn get_stream_state(&self, chat_id: i64) -> Option<StreamStateInfo> {
    let active = self.active_streams.lock().await;
    active.get(&chat_id).map(|state| StreamStateInfo::from(state))
}

pub async fn is_aborted(&self, chat_id: i64) -> bool {
    let active = self.active_streams.lock().await;
    active.get(&chat_id).map(|s| s.is_aborted()).unwrap_or(false)
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

**Add `StreamStateInfo` struct for external queries:**

```rust
#[derive(Debug, Clone)]
pub struct StreamStateInfo {
    pub is_running: bool,
    pub is_paused: bool,
    pub is_aborted: bool,
    pub phase: Option<ExecutionPhase>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub aborted_tool_ids: Vec<String>,
}

impl From<&StreamState> for StreamStateInfo {
    fn from(state: &StreamState) -> Self {
        match state {
            StreamState::Running { phase, .. } => StreamStateInfo {
                is_running: true,
                is_paused: false,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: Vec::new(),
            },
            StreamState::Paused { phase, pending_tool_calls, .. } => StreamStateInfo {
                is_running: false,
                is_paused: true,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: pending_tool_calls.clone(),
                aborted_tool_ids: Vec::new(),
            },
            StreamState::Aborted { aborted_tool_ids, .. } => StreamStateInfo {
                is_running: false,
                is_paused: false,
                is_aborted: true,
                phase: None,
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: aborted_tool_ids.clone(),
            },
        }
    }
}
```

**Update `check_pause_state()` to handle new state:**

```rust
pub(crate) async fn check_pause_state(
    &self,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) {
    let pause_notify = {
        let active = self.active_streams.lock().await;
        if let Some(StreamState::Paused { pause_notify, .. }) = active.get(&chat_id) {
            Some(pause_notify.clone())
        } else {
            None
        }
    };

    if let Some(notify) = pause_notify {
        let _ = event_sender.send(ChatEvent::ChatPaused { chat_id });
        notify.notified().await;
        let _ = event_sender.send(ChatEvent::ChatResumed { chat_id });
    }
}
```

### 3. `packages/rhd_chat/src/lib.rs`

**Export new types:**

```rust
pub use state::{ExecutionPhase, PendingToolCall, StreamState, StreamStateInfo};
```

## Tests

### Unit Tests

Add to `packages/rhd_chat/src/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_state_is_aborted() {
        let cancel_token = CancellationToken::new();
        let aborted = StreamState::Aborted {
            cancel_token,
            aborted_tool_ids: vec!["call_1".to_string()],
        };
        assert!(aborted.is_aborted());

        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let running = StreamState::Running {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::AiCall,
        };
        assert!(!running.is_aborted());
    }

    #[test]
    fn test_stream_state_phase() {
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let running = StreamState::Running {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::ToolExecution,
        };
        assert_eq!(running.phase(), Some(&ExecutionPhase::ToolExecution));
    }

    #[test]
    fn test_stream_state_pending_tool_calls() {
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let pending = vec![PendingToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }];
        let paused = StreamState::Paused {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::AiCall,
            pending_tool_calls: pending.clone(),
        };
        assert_eq!(paused.pending_tool_calls(), Some(&pending));
    }
}
```

Add to `packages/rhd_chat/src/manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ExecutionPhase;

    #[tokio::test]
    async fn test_pause_chat_with_pending_tools() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        let pending_tools = vec![PendingToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }];

        let result = manager.pause_chat(chat_id, pending_tools.clone()).await;
        assert!(result);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_paused);
        assert_eq!(state.pending_tool_calls.len(), 1);
        assert_eq!(state.pending_tool_calls[0].id, "call_1");
    }

    #[tokio::test]
    async fn test_abort_chat_transitions_to_aborted() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        let aborted_tools = vec!["call_1".to_string(), "call_2".to_string()];
        let result = manager.abort_chat(chat_id, aborted_tools.clone()).await;
        assert!(result);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_aborted);
        assert_eq!(state.aborted_tool_ids, aborted_tools);
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test]
    async fn test_resume_chat_returns_pending_tools() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, pause_notify) = manager.register_stream(chat_id).await;

        let pending_tools = vec![PendingToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }];

        manager.pause_chat(chat_id, pending_tools.clone()).await;
        let result = manager.resume_chat(chat_id).await;
        assert!(result.is_some());

        let (returned_tools, phase) = result.unwrap();
        assert_eq!(returned_tools.len(), 1);
        assert_eq!(returned_tools[0].id, "call_1");
        assert_eq!(phase, ExecutionPhase::AiCall);

        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert!(state.is_running);
        assert!(!state.is_paused);
    }
}
```

## Implementation Notes

1. **Three-state model**: The new `StreamState` has three variants (`Running`, `Paused`, `Aborted`) instead of two. This allows the system to distinguish between "paused and will resume" and "aborted and needs cleanup".

2. **Execution phase tracking**: The `ExecutionPhase` enum tracks whether we're in the AI call phase or tool execution phase. This is critical for determining the correct pause/abort behavior.

3. **Pending tool calls storage**: When pausing during AI call, tool calls are stored in `pending_tool_calls` without being executed. On resume, these tool calls are executed first.

4. **Aborted tool tracking**: When aborting during tool execution, the IDs of aborted tools are stored in `aborted_tool_ids`. This allows the frontend to show "Aborted" error for those specific tools.

5. **Backward compatibility**: The `check_pause_state()` method signature remains the same, but internally it now handles the new state structure.

6. **Thread safety**: All state transitions are protected by the `Mutex<HashMap<i64, StreamState>>` lock, ensuring thread-safe access.

## Dependencies

- This phase depends on: None (first phase)
- This phase must be completed before: Phase 2 (Tool Loop Integration)
- Phase 3 (Message Queue) can start after this phase
