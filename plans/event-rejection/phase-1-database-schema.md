# Phase 1: Database Schema Changes

## Overview

This phase extends the database schema to support storing the new context fields (`chat_id`, `message_id`, `tool_call_id`) for custom events. This is the foundational layer that enables all subsequent functionality.

**Scope**: Database schema migration and data access layer updates
**Out of scope**: API types, server logic, client code

## Files to Modify

### 1. `packages/rhd_db/src/chat_db/schema.rs`

**Add migration logic** (after line 246, before the closing `Ok(())`):

```rust
// Check if chat_id column exists in custom_events table
let has_chat_id: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('custom_events') WHERE name='chat_id'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_chat_id {
    conn.execute_batch("ALTER TABLE custom_events ADD COLUMN chat_id TEXT")?;
}

// Check if message_id column exists in custom_events table
let has_message_id: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('custom_events') WHERE name='message_id'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_message_id {
    conn.execute_batch("ALTER TABLE custom_events ADD COLUMN message_id TEXT")?;
}

// Check if tool_call_id column exists in custom_events table
let has_tool_call_id: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('custom_events') WHERE name='tool_call_id'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_tool_call_id {
    conn.execute_batch("ALTER TABLE custom_events ADD COLUMN tool_call_id TEXT")?;
}
```

**Why**: Adds three nullable TEXT columns to the `custom_events` table. These fields are optional and informational only - no foreign key constraints are added.

### 2. `packages/rhd_db/src/chat_db/custom_events.rs`

**Modify `CustomEventInfo` struct** (lines 7-15):

```rust
/// Custom event information from the database.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventInfo {
    pub event_id: String,
    pub event_name: String,
    pub sender_plugin_id: Option<String>,
    pub additional: Option<String>,
    pub chat_id: Option<String>,
    pub message_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}
```

**Modify `create_custom_event` function** (lines 18-31):

```rust
/// Create a new custom event.
pub fn create_custom_event(
    conn: &Mutex<Connection>,
    event_id: &str,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
    chat_id: Option<&str>,
    message_id: Option<&str>,
    tool_call_id: Option<&str>,
) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "INSERT INTO custom_events (event_id, event_name, sender_plugin_id, additional, chat_id, message_id, tool_call_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![event_id, event_name, sender_plugin_id, additional, chat_id, message_id, tool_call_id],
    )?;
    Ok(())
}
```

**Modify `get_custom_event` function** (lines 34-52):

```rust
/// Get a custom event by ID.
pub fn get_custom_event(conn: &Mutex<Connection>, event_id: &str) -> DbResult<Option<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT event_id, event_name, sender_plugin_id, additional, chat_id, message_id, tool_call_id, created_at FROM custom_events WHERE event_id = ?"
    )?;
    let event = stmt
        .query_map([event_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                chat_id: row.get::<_, Option<String>>(4)?,
                message_id: row.get::<_, Option<String>>(5)?,
                tool_call_id: row.get::<_, Option<String>>(6)?,
                created_at: row.get::<_, String>(7)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(event)
}
```

**Modify `get_pending_events_for_plugin` function** (lines 74-96):

```rust
/// Get all custom events that a plugin has NOT acknowledged.
pub fn get_pending_events_for_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<Vec<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT ce.event_id, ce.event_name, ce.sender_plugin_id, ce.additional, ce.chat_id, ce.message_id, ce.tool_call_id, ce.created_at
         FROM custom_events ce
         WHERE ce.event_id NOT IN (
             SELECT event_id FROM custom_event_acks WHERE plugin_id = ?
         )
         ORDER BY ce.created_at"
    )?;
    let events = stmt
        .query_map([plugin_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                chat_id: row.get::<_, Option<String>>(4)?,
                message_id: row.get::<_, Option<String>>(5)?,
                tool_call_id: row.get::<_, Option<String>>(6)?,
                created_at: row.get::<_, String>(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}
```

**Why**: Updates all database access functions to handle the new context fields. The fields are optional and passed through without validation.

### 3. `packages/rhd_db/src/chat_db/mod.rs`

**Modify `create_custom_event` method** (lines 219-227):

```rust
pub fn create_custom_event(
    &self,
    event_id: &str,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
    chat_id: Option<&str>,
    message_id: Option<&str>,
    tool_call_id: Option<&str>,
) -> DbResult<()> {
    custom_events::create_custom_event(&self.conn, event_id, event_name, sender_plugin_id, additional, chat_id, message_id, tool_call_id)
}
```

**Why**: Updates the public API to accept the new context fields.

### 4. `packages/rhd_db/src/chat_db/tests/tags_plugins_events_tests.rs`

**Add new test** (after line 135):

```rust
#[test]
fn test_custom_events_with_context_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    
    // Create custom event with context fields
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        Some("{\"key\": \"value\"}"),
        Some("chat-123"),
        Some("message-456"),
        Some("call-789"),
    ).unwrap();
    
    // Get event and verify context fields
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.event_id, "event-1");
    assert_eq!(event.event_name, "my-event");
    assert_eq!(event.sender_plugin_id, Some("sender-plugin".to_string()));
    assert_eq!(event.chat_id, Some("chat-123".to_string()));
    assert_eq!(event.message_id, Some("message-456".to_string()));
    assert_eq!(event.tool_call_id, Some("call-789".to_string()));
    
    // Check pending events include context fields
    db.register_plugin("receiver-plugin").unwrap();
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].chat_id, Some("chat-123".to_string()));
    assert_eq!(pending[0].message_id, Some("message-456".to_string()));
    assert_eq!(pending[0].tool_call_id, Some("call-789".to_string()));
}

#[test]
fn test_custom_events_without_context_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    
    // Create custom event without context fields (backward compatibility)
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        Some("{\"key\": \"value\"}"),
        None,
        None,
        None,
    ).unwrap();
    
    // Get event and verify context fields are None
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.chat_id, None);
    assert_eq!(event.message_id, None);
    assert_eq!(event.tool_call_id, None);
}
```

**Why**: Tests verify that the new fields are stored and retrieved correctly, and that backward compatibility is maintained (fields can be None).

## Implementation Notes

1. **Nullable columns**: All three fields are optional, so they must be nullable in the schema
2. **No foreign key constraints**: The fields are informational only; we don't validate that the referenced chat/message/tool_call exists
3. **Additive migration**: The schema change adds columns without modifying existing ones, ensuring backward compatibility
4. **Column order**: The new columns are added after `additional` and before `created_at` in the SELECT statements

## Dependencies

- **None** - This is the foundational phase
- This phase must be completed before Phase 2 (API Types)

## Success Criteria

- ✅ Database schema includes three new nullable columns in `custom_events` table
- ✅ `create_custom_event()` accepts and stores the new fields
- ✅ `get_custom_event()` retrieves the new fields
- ✅ `get_pending_events_for_plugin()` includes the new fields in results
- ✅ Existing tests continue to pass (backward compatibility)
- ✅ New tests verify storage and retrieval of the new fields
- ✅ New tests verify backward compatibility (fields can be None)
