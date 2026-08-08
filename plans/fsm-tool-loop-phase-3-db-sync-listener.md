# Phase 3: DB Sync Listener

## Overview

This phase implements a DB synchronization listener that keeps the database in sync with FSM state. The listener receives events from the FSM and performs corresponding database operations.

**Scope:**
- Create `create_db_sync_listener()` function that returns a listener callback
- Handle all message-related events (`MessageInserted`, `MessageRemoved`, `MessageReplaced`, `AllMessagesReplaced`)
- Handle `ToolCallIdGenerated` event (if needed for DB counter updates)
- Ensure atomic updates for consistency
- Handle message ID mapping between FSM and DB

**Out of Scope:**
- Async wrapper implementation (Phase 2)
- Replacement of existing `tool_loop()` function (Phase 4)
- Helper FSMs (Phase 6)

## Files to Create

### 1. `packages/rhd_chat/src/tools/db_sync_listener.rs` (NEW FILE)

**Purpose:** Implement DB synchronization listener that reacts to FSM events and updates the database.

**Complete Implementation:**

```rust
use std::sync::Arc;

use rhd_db::ChatDb;
use rhd_fsm::{ToolLoopFsmEvent, ToolLoopListenerCallback};
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;

/// Creates a DB synchronization listener callback
///
/// This listener receives FSM events and performs corresponding database operations
/// to keep the database in sync with the FSM's state.
///
/// # Arguments
///
/// * `db` - Arc reference to the ChatDb instance
/// * `chat_id` - The ID of the chat being synchronized
/// * `event_sender` - Broadcast sender for emitting ChatEvents to WebSocket clients
///
/// # Returns
///
/// A `ToolLoopListenerCallback` that can be registered with the FSM
pub fn create_db_sync_listener(
    db: Arc<ChatDb>,
    chat_id: i64,
    event_sender: broadcast::Sender<ChatEvent>,
) -> ToolLoopListenerCallback {
    Arc::new(move |event: ToolLoopFsmEvent| {
        let result = handle_event(&db, chat_id, &event_sender, &event);
        if let Err(e) = result {
            eprintln!("DB sync listener error: {}", e);
        }
    })
}

/// Handle a single FSM event and perform corresponding DB operations
fn handle_event(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    event: &ToolLoopFsmEvent,
) -> Result<(), ChatError> {
    match event {
        ToolLoopFsmEvent::MessageInserted { message } => {
            handle_message_inserted(db, chat_id, event_sender, message)
        }
        ToolLoopFsmEvent::MessageRemoved { message_id } => {
            handle_message_removed(db, chat_id, event_sender, *message_id)
        }
        ToolLoopFsmEvent::MessageReplaced { message_id, new_message } => {
            handle_message_replaced(db, chat_id, event_sender, *message_id, new_message)
        }
        ToolLoopFsmEvent::AllMessagesReplaced { messages } => {
            handle_all_messages_replaced(db, chat_id, event_sender, messages)
        }
        ToolLoopFsmEvent::ToolCallIdGenerated { tool_call_id: _ } => {
            // Tool call IDs are managed by FSM, no DB sync needed
            Ok(())
        }
        ToolLoopFsmEvent::ToolCallRequested { .. } => {
            // Tool call requests are handled by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::ToolCallExecuted { .. } => {
            // Tool call execution is handled by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::AiResponseReceived { .. } => {
            // AI response handling is done by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::StateChanged { .. } => {
            // State changes don't require DB updates
            Ok(())
        }
    }
}

/// Handle MessageInserted event - add message to DB
fn handle_message_inserted(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message: &rhd_fsm::ChatMessage,
) -> Result<(), ChatError> {
    // Convert FSM message to DB message
    let db_message = convert_fsm_message_to_db(message, chat_id);

    // Insert message into DB
    let inserted = db.insert_message(&db_message)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: convert_db_message_to_api(&inserted),
    });

    Ok(())
}

/// Handle MessageRemoved event - remove message from DB
fn handle_message_removed(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message_id: i64,
) -> Result<(), ChatError> {
    // Remove message from DB
    db.delete_message(message_id)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageRemoved {
        chat_id,
        message_id,
    });

    Ok(())
}

/// Handle MessageReplaced event - update message in DB
fn handle_message_replaced(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message_id: i64,
    new_message: &rhd_fsm::ChatMessage,
) -> Result<(), ChatError> {
    // Convert FSM message to DB message
    let db_message = convert_fsm_message_to_db(new_message, chat_id);

    // Update message in DB
    let updated = db.update_message(&db_message)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageReplaced {
        chat_id,
        message: convert_db_message_to_api(&updated),
    });

    Ok(())
}

/// Handle AllMessagesReplaced event - truncate and re-add all messages
fn handle_all_messages_replaced(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    messages: &[rhd_fsm::ChatMessage],
) -> Result<(), ChatError> {
    // Delete all existing messages for this chat
    db.delete_all_messages(chat_id)?;

    // Insert all new messages
    for message in messages {
        let db_message = convert_fsm_message_to_db(message, chat_id);
        let inserted = db.insert_message(&db_message)?;

        // Emit ChatEvent for each inserted message
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: convert_db_message_to_api(&inserted),
        });
    }

    Ok(())
}

/// Convert FSM ChatMessage to DB ChatMessage
fn convert_fsm_message_to_db(
    fsm_message: &rhd_fsm::ChatMessage,
    chat_id: i64,
) -> rhd_db::ChatMessage {
    rhd_db::ChatMessage {
        id: fsm_message.id,
        chat_id,
        role: fsm_message.role.clone(),
        content: fsm_message.content.clone(),
        thinking_content: fsm_message.thinking_content.clone(),
        tool_calls: fsm_message.tool_calls.as_ref().map(|tcs| {
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
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Convert DB ChatMessage to API ChatMessage for WebSocket events
fn convert_db_message_to_api(db_message: &rhd_db::ChatMessage) -> crate::state::ChatMessage {
    crate::state::ChatMessage {
        id: db_message.id,
        chat_id: db_message.chat_id,
        role: db_message.role.clone(),
        content: db_message.content.clone(),
        thinking_content: db_message.thinking_content.clone(),
        tool_calls: db_message.tool_calls.as_ref().map(|tcs| {
            tcs.iter()
                .map(|tc| crate::state::ToolCall {
                    id: tc.id.clone(),
                    function: crate::state::FunctionCall {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                })
                .collect()
        }),
        model: db_message.model.clone(),
        created_at: db_message.created_at.clone(),
    }
}
```

### 2. `packages/rhd_chat/src/tools/mod.rs`

**Modifications:**

Add export for the new module:

```rust
pub mod db_sync_listener;
```

## Files to Modify

### 3. `packages/rhd_db/src/chat_db/messages.rs`

**Modifications:**

Add methods for message operations if they don't exist:

```rust
impl ChatDb {
    /// Insert a new message into the database
    pub fn insert_message(&self, message: &ChatMessage) -> Result<ChatMessage, ChatDbError> {
        // Implementation depends on existing schema
        // This is a placeholder - actual implementation should match existing patterns
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO messages (id, chat_id, role, content, thinking_content, tool_calls, model, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                message.id,
                message.chat_id,
                message.role,
                message.content,
                message.thinking_content,
                message.tool_calls.as_ref().map(|tc| serde_json::to_string(tc).unwrap()),
                message.model,
                message.created_at,
            ],
        )?;
        
        Ok(message.clone())
    }

    /// Update an existing message in the database
    pub fn update_message(&self, message: &ChatMessage) -> Result<ChatMessage, ChatDbError> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE messages SET role = ?1, content = ?2, thinking_content = ?3, 
             tool_calls = ?4, model = ?5 WHERE id = ?6",
            rusqlite::params![
                message.role,
                message.content,
                message.thinking_content,
                message.tool_calls.as_ref().map(|tc| serde_json::to_string(tc).unwrap()),
                message.model,
                message.id,
            ],
        )?;
        
        Ok(message.clone())
    }

    /// Delete a message from the database
    pub fn delete_message(&self, message_id: i64) -> Result<(), ChatDbError> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute("DELETE FROM messages WHERE id = ?1", rusqlite::params![message_id])?;
        
        Ok(())
    }

    /// Delete all messages for a chat
    pub fn delete_all_messages(&self, chat_id: i64) -> Result<(), ChatDbError> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute("DELETE FROM messages WHERE chat_id = ?1", rusqlite::params![chat_id])?;
        
        Ok(())
    }

    /// Get the next message ID for a chat (for FSM initialization)
    pub fn get_next_message_id(&self, chat_id: i64) -> Result<i64, ChatDbError> {
        let conn = self.conn.lock().unwrap();
        
        let max_id: Option<i64> = conn.query_row(
            "SELECT MAX(id) FROM messages WHERE chat_id = ?1",
            rusqlite::params![chat_id],
            |row| row.get(0),
        )?;
        
        Ok(max_id.map(|id| id + 1).unwrap_or(1_000_000))
    }
}
```

**Note:** The actual implementation should match the existing database schema and patterns in the codebase. The above is a template that needs to be adapted to the actual schema.

## Tests

### Unit Tests

Create `packages/rhd_chat/src/tools/tests/db_sync_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rhd_fsm::{ChatMessage, ToolLoopFsmEvent};
    use std::sync::Arc;
    use tokio::sync::broadcast;

    #[test]
    fn test_message_inserted_event() {
        // Test that MessageInserted event triggers DB insert
        // This requires a test database setup
    }

    #[test]
    fn test_message_removed_event() {
        // Test that MessageRemoved event triggers DB delete
    }

    #[test]
    fn test_message_replaced_event() {
        // Test that MessageReplaced event triggers DB update
    }

    #[test]
    fn test_all_messages_replaced_event() {
        // Test that AllMessagesReplaced event triggers bulk update
    }

    #[test]
    fn test_convert_fsm_message_to_db() {
        let fsm_message = ChatMessage {
            id: 1,
            role: "user".to_string(),
            content: "Hello".to_string(),
            thinking_content: None,
            tool_calls: None,
        };

        let db_message = convert_fsm_message_to_db(&fsm_message, 100);
        
        assert_eq!(db_message.id, 1);
        assert_eq!(db_message.chat_id, 100);
        assert_eq!(db_message.role, "user");
        assert_eq!(db_message.content, "Hello");
    }
}
```

## Implementation Notes

1. **Synchronous Listener**: The listener callback is synchronous (`Fn` not `async Fn`) because the FSM is synchronous. Database operations must be non-blocking or use a connection pool.

2. **Error Handling**: Errors in the listener are logged but don't stop the FSM. This ensures that DB sync issues don't break the tool loop.

3. **Message ID Mapping**: The FSM generates message IDs using its internal counter. The DB listener uses these IDs directly, ensuring consistency between FSM and DB.

4. **Event Ordering**: Events are emitted in the order they occur in the FSM. The listener processes them sequentially, maintaining consistency.

5. **WebSocket Notifications**: The listener emits `ChatEvent` messages to notify WebSocket clients of database changes. This keeps the frontend in sync.

6. **Atomic Operations**: For `AllMessagesReplaced`, the listener deletes all messages and re-inserts them. This should be wrapped in a transaction for atomicity (implementation depends on DB layer).

7. **Tool Call Serialization**: Tool calls are serialized to JSON for storage in the database. The format must match the existing schema.

## Dependencies

- This phase depends on Phase 1 (FSM Listener System) for the event system
- This phase must be completed before Phase 4 (Replace tool_loop)
- This phase can be done in parallel with Phase 2 (Async Wrapper)

## Success Criteria

- [ ] DB stays in sync with FSM state
- [ ] Message IDs are correctly mapped between FSM and DB
- [ ] WebSocket events are emitted for all message changes
- [ ] Concurrent access handled correctly (if applicable)
- [ ] Unit tests for DB sync pass
- [ ] Code compiles without warnings
- [ ] Error handling is robust (DB errors don't crash FSM)
