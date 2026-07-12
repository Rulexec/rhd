# Phase 4b: Pause/Resume Functionality

## Goal
Add ability to pause and resume tool execution loop during chat conversations.

## Current State Analysis
- Phase 4a provides basic tool execution loop
- [`active_streams`](packages/rhd_app/src/chat.rs:40) tracks `HashMap<i64, CancellationToken>` for abort
- Need to extend this to support pause/resume state machine
- Pause happens between tool calls, not mid-execution (ensures tool results are complete)

## Subtasks

### 4b.1. Extend active_streams to track pause state
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Current**:
```rust
active_streams: Mutex<HashMap<i64, CancellationToken>>
```

**New**:
```rust
enum StreamState {
    Running(CancellationToken),
    Paused {
        cancel_token: CancellationToken,
        resume_tx: oneshot::Sender<()>,
    },
}

active_streams: Mutex<HashMap<i64, StreamState>>
```

**Design decision**: Pause happens between tool calls, not mid-execution. This ensures tool results are complete before pausing.

### 4b.2. Implement pause/resume methods in ChatManager
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**New methods**:
```rust
pub async fn pause_chat(&self, chat_id: i64) -> bool
```
- Check if chat has active stream in Running state
- Create oneshot channel for resume signal
- Change state to Paused
- Return true if paused, false if not running

```rust
pub async fn resume_chat(&self, chat_id: i64) -> bool
```
- Check if chat has active stream in Paused state
- Send signal via resume_tx channel
- Change state back to Running
- Return true if resumed, false if not paused

**Note**: `cancel_chat()` already exists, reuse for cancel.

### 4b.3. Modify tool execution loop to support pause
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Logic**:
- In `send_message_with_tools()`, after each tool call completes:
  - Check if stream state is Paused
  - If paused, wait for resume signal via oneshot channel
  - When signal received, continue loop
- Pause check happens between tool calls, not during tool execution

**Implementation**:
```rust
// After tool call completed
loop {
    // Check if paused
    let state = active_streams.lock().await.get(&chat_id).cloned();
    if let Some(StreamState::Paused { resume_rx, .. }) = state {
        let _ = event_sender.send(ChatEvent::ChatPaused { chat_id });
        let _ = resume_rx.await;  // Wait for resume signal
        let _ = event_sender.send(ChatEvent::ChatResumed { chat_id });
    }
    
    // Continue with next tool call or AI request
}
```

### 4b.4. Add WebSocket API for pause/resume
**File**: [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs:150)

**New WsRequest variants**:
```rust
#[serde(rename = "pauseChat", rename_all = "camelCase")]
PauseChat { id: String, chat_id: i64 },

#[serde(rename = "resumeChat", rename_all = "camelCase")]
ResumeChat { id: String, chat_id: i64 },
```

**File**: [`packages/rhd_app/src/ws.rs`](packages/rhd_app/src/ws.rs)

**Handler logic**:
- `PauseChat` → call `chat_manager.pause_chat()`, return success
- `ResumeChat` → call `chat_manager.resume_chat()`, return success

### 4b.5. Add chat events for pause/resume
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs:30)

**New ChatEvent variants**:
```rust
ChatPaused { chat_id: i64 },
ChatResumed { chat_id: i64 },
```

**WebSocket events**:
- `chatPaused` - UI shows pause state, enables resume button
- `chatResumed` - UI hides pause state

## Deliverables
- [ ] Pause/resume state machine in [`ChatManager`](packages/rhd_app/src/chat.rs:38)
- [ ] `pause_chat()` / `resume_chat()` methods
- [ ] Tool loop pause check between tool calls
- [ ] WebSocket API: `pauseChat`, `resumeChat`
- [ ] WebSocket events: `chatPaused`, `chatResumed`
- [ ] Unit tests for pause/resume

## Dependencies
- Phase 4a (Basic tool loop) must be complete

## Risk Assessment
- **Medium risk**: State machine complexity - must handle edge cases
- **Unknown**: How to handle pause during AI streaming (not tool execution)
- **Mitigation**: 
  - Pause only between tool calls, not mid-execution
  - Clear state transitions (Running ↔ Paused)
  - Cancel always works (even when paused)
