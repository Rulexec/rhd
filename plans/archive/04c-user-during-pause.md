# Phase 4c: User Message During Pause

## Goal
Allow users to send messages while tool execution is paused, enabling interactive guidance of AI behavior.

## Current State Analysis
- Phase 4b provides pause/resume functionality
- When paused, tool loop waits for resume signal
- Need to allow user to send message during pause
- Message should be added to history and loop should resume with updated context

## Subtasks

### 4c.1. Modify send_message() to handle paused state
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Current behavior**:
- `send_message()` checks if chat exists and model is valid
- Adds user message to DB
- Starts new AI stream

**New behavior**:
- Check if chat has active stream in Paused state
- If paused:
  - Add user message to DB
  - Send resume signal to wake up paused tool loop
  - Return immediately (no new stream started)
- If not paused:
  - Normal flow (add message, start new stream)

**Implementation**:
```rust
pub async fn send_message(
    &self,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<i64, ChatError> {
    // Check if paused
    let state = self.active_streams.lock().await.get(&chat_id).cloned();
    if let Some(StreamState::Paused { resume_tx, .. }) = state {
        // Add user message to DB
        let user_message_id = self.db.add_message(chat_id, "user", &content, Some(model))?;
        
        // Emit message added event
        let user_message = Message { /* ... */ };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: user_message,
        });
        
        // Resume the paused loop
        let _ = resume_tx.send(());
        
        return Ok(user_message_id);
    }
    
    // Normal flow...
}
```

### 4c.2. Update tool loop to incorporate user messages
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Logic**:
- When tool loop resumes after pause:
  - Fetch updated message history from DB (includes new user message)
  - Continue loop with updated context
  - AI will see user message and can respond accordingly

**Implementation detail**:
- After resume signal received, reload messages from DB
- Build new message history including user message
- Continue tool loop with updated history

### 4c.3. Handle edge cases
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Edge cases**:
1. **Multiple user messages during pause**: Each message adds to DB, but only one resume signal needed
2. **User message during Running state**: Normal flow (not paused)
3. **User message during AI streaming**: Wait for stream to complete, then add message
4. **Pause during user message send**: Should not happen (send is fast)

**Mitigation**:
- Check state before adding message
- If state changes during send, handle gracefully
- Ensure message ordering is correct

### 4c.4. Add WebSocket event for user message during pause
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs:30)

**Optional**: Add event to indicate user message was added during pause
```rust
UserMessageDuringPause { chat_id: i64, message_id: i64 },
```

**WebSocket event**:
- `userMessageDuringPause` - UI can show indicator that message will guide AI

**Note**: This is optional. The existing `messageAdded` event is sufficient for UI updates.

## Deliverables
- [ ] Modified `send_message()` to handle paused state
- [ ] Tool loop reloads history after resume
- [ ] Edge case handling (multiple messages, state changes)
- [ ] Unit tests for user message during pause

## Dependencies
- Phase 4b (Pause/resume) must be complete

## Risk Assessment
- **Medium risk**: State machine complexity - must handle race conditions
- **Unknown**: How to handle very rapid user messages during pause
- **Mitigation**: 
  - Check state before adding message
  - Ensure message ordering via DB timestamps
  - Resume signal is idempotent (multiple sends OK)

## UX Considerations
- User sees pause indicator in UI
- User can type and send message while paused
- Message appears in chat history immediately
- Tool loop resumes automatically after message added
- AI sees user message and can adjust behavior
