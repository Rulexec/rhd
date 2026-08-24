# Phase 1: API Types & Database Schema

## Overview

Add `is_finished` and `is_streaming` flags to the `Message` type in both `rhd_chat_api` and `rhd_db`, and add corresponding database columns via migration. This is the foundation phase — all subsequent phases depend on these types.

## Scope

- Add `is_finished: bool` (default `true`) and `is_streaming: bool` (default `false`) to `Message` in `rhd_chat_api::common`
- Add same fields to `Message` in `rhd_db::chat_db::mod`
- Add database columns with backward-compatible defaults
- Update `add_message` and `update_message` in DB layer to handle new fields
- Update `AddMessageParams` and `UpdateMessageParams` to accept new fields

## Dependencies

- None — this is the foundation phase.

---

## Files to Modify

### 1. `packages/rhd_chat_api/src/common.rs`

**Modify `Message` struct (line 58):**

Add two new fields after `tags`:

```rust
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether the message content is final (no more updates expected).
    #[serde(default = "default_true")]
    pub is_finished: bool,
    /// Whether the message is currently being streamed.
    #[serde(default)]
    pub is_streaming: bool,
}
```

**Add helper function** (before the `Message` struct):

```rust
fn default_true() -> bool {
    true
}
```

**Update doc comment** for `Message` to include new fields in the example JSON:

```rust
/// # Example JSON
/// ```json
/// {
///   "id": 456,
///   "chatId": 123,
///   "role": "user",
///   "content": "Hello",
///   "createdAt": "2026-08-20T18:00:00Z",
///   "reasoningContent": null,
///   "tags": ["important"],
///   "isFinished": true,
///   "isStreaming": false
/// }
/// ```
```

**Update tests** in the same file:
- `test_message_serialization`: Add `is_finished: true, is_streaming: false` to the test message and assert the JSON contains `"isFinished":true` and `"isStreaming":false`.
- `test_message_with_reasoning_content`: Same additions.

---

### 2. `packages/rhd_db/src/chat_db/mod.rs`

**Modify `Message` struct (line 35):**

Add two new fields:

```rust
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub model: Option<String>,
    pub thinking_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub is_finished: bool,
    pub is_streaming: bool,
}
```

**Modify `add_message` method (line 113):**

Add `is_finished` and `is_streaming` parameters:

```rust
pub fn add_message(
    &self,
    chat_id: i64,
    role: &str,
    content: &str,
    model: Option<&str>,
    thinking_content: Option<&str>,
    is_finished: bool,
    is_streaming: bool,
) -> DbResult<(i64, i64)> {
    messages::add_message(&self.conn, chat_id, role, content, model, thinking_content, is_finished, is_streaming)
}
```

**Modify `update_message` method (line 132):**

Expand to support updating all message fields:

```rust
pub fn update_message(
    &self,
    message_id: i64,
    content: Option<&str>,
    thinking_content: Option<&str>,
    tool_calls: Option<&str>,
    is_finished: Option<bool>,
    is_streaming: Option<bool>,
) -> DbResult<i64> {
    messages::update_message(&self.conn, message_id, content, thinking_content, tool_calls, is_finished, is_streaming)
}
```

---

### 3. `packages/rhd_db/src/chat_db/messages.rs`

**Modify `add_message` function (line 12):**

Add `is_finished` and `is_streaming` parameters:

```rust
pub(crate) fn add_message(
    conn: &Mutex<Connection>,
    chat_id: i64,
    role: &str,
    content: &str,
    model: Option<&str>,
    thinking_content: Option<&str>,
    is_finished: bool,
    is_streaming: bool,
) -> DbResult<(i64, i64)> {
    let conn = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let now = now_iso();
    tx.execute(
        "INSERT INTO messages (chat_id, role, content, created_at, model, thinking_content, is_finished, is_streaming) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![chat_id, role, content, now, model, thinking_content, is_finished, is_streaming],
    )?;
    let message_id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok((message_id, new_version))
}
```

**Modify `get_messages` function (line 43):**

Update SQL query and row mapping to include new fields:

```rust
pub(crate) fn get_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Vec<Message>> {
    let conn = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls, is_finished, is_streaming FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![chat_id], |row| {
        let tool_calls_json: Option<String> = row.get(7)?;
        let tool_calls = tool_calls_json
            .and_then(|json| serde_json::from_str::<Vec<ToolCall>>(&json).ok());
        Ok(Message {
            id: row.get(0)?,
            chat_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            model: row.get(5)?,
            thinking_content: row.get(6)?,
            tool_calls,
            is_finished: row.get::<_, bool>(8)?,
            is_streaming: row.get::<_, bool>(9)?,
        })
    })?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}
```

**Modify `get_message` function (line 72):**

Same pattern as `get_messages` — add `is_finished` and `is_streaming` to SELECT and row mapping.

**Rewrite `update_message` function (line 100):**

Replace the current simple content-only update with a partial update supporting all fields:

```rust
pub(crate) fn update_message(
    conn: &Mutex<Connection>,
    message_id: i64,
    content: Option<&str>,
    thinking_content: Option<&str>,
    tool_calls: Option<&str>,
    is_finished: Option<bool>,
    is_streaming: Option<bool>,
) -> DbResult<i64> {
    let conn = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages WHERE id = ?1",
        params![message_id],
        |row| row.get(0),
    )?;

    // Build dynamic UPDATE based on which fields are Some
    if let Some(content) = content {
        tx.execute("UPDATE messages SET content = ?1 WHERE id = ?2", params![content, message_id])?;
    }
    if let Some(thinking_content) = thinking_content {
        tx.execute("UPDATE messages SET thinking_content = ?1 WHERE id = ?2", params![thinking_content, message_id])?;
    }
    if let Some(tool_calls) = tool_calls {
        tx.execute("UPDATE messages SET tool_calls = ?1 WHERE id = ?2", params![tool_calls, message_id])?;
    }
    if let Some(is_finished) = is_finished {
        tx.execute("UPDATE messages SET is_finished = ?1 WHERE id = ?2", params![is_finished, message_id])?;
    }
    if let Some(is_streaming) = is_streaming {
        tx.execute("UPDATE messages SET is_streaming = ?1 WHERE id = ?2", params![is_streaming, message_id])?;
    }

    let now = now_iso();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}
```

---

### 4. `packages/rhd_db/src/chat_db/schema.rs`

**Add migration** in the `migrate` function (after the existing `version` migration):

```rust
// Check if is_finished column exists in messages table
let has_is_finished: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='is_finished'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_is_finished {
    conn.execute_batch("ALTER TABLE messages ADD COLUMN is_finished INTEGER NOT NULL DEFAULT 1")?;
}

// Check if is_streaming column exists in messages table
let has_is_streaming: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='is_streaming'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_is_streaming {
    conn.execute_batch("ALTER TABLE messages ADD COLUMN is_streaming INTEGER NOT NULL DEFAULT 0")?;
}
```

---

### 5. `packages/rhd_chat_api/src/methods/add_message.rs`

**Modify `AddMessageParams` struct:**

Add optional streaming fields:

```rust
pub struct AddMessageParams {
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Whether the message is finished (default: true).
    #[serde(default = "default_true")]
    pub is_finished: bool,
    /// Whether the message is being streamed (default: false).
    #[serde(default)]
    pub is_streaming: bool,
}
```

Add `fn default_true() -> bool { true }` at module level.

**Update tests** to cover new fields.

---

### 6. `packages/rhd_chat_api/src/methods/update_message.rs`

**Modify `UpdateMessageParams` struct:**

Add optional streaming fields:

```rust
pub struct UpdateMessageParams {
    pub message_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_tags: Vec<String>,
    /// Set whether the message is finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_finished: Option<bool>,
    /// Set whether the message is being streamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_streaming: Option<bool>,
    /// Set tool calls (JSON string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,
}
```

**Update tests** to cover new fields.

---

### 7. `packages/rhd_chat_server/src/handlers/message.rs`

**Modify `add_message` handler (line 40):**

Pass new fields to `db.add_message`:

```rust
let (message_id, chat_version) = db.add_message(
    params.chat_id,
    &params.role,
    &params.content,
    None, // model
    params.reasoning_content.as_deref(),
    params.is_finished,
    params.is_streaming,
)?;
```

**Modify `convert_message_to_api` function (line 19):**

Add new fields to the conversion:

```rust
fn convert_message_to_api(
    msg: rhd_db::Message,
    tags: Vec<String>,
) -> Result<rhd_chat_api::Message, ServerError> {
    let created_at: DateTime<Utc> = msg
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;

    Ok(rhd_chat_api::Message {
        id: msg.id,
        chat_id: msg.chat_id,
        role: msg.role,
        content: msg.content,
        created_at,
        reasoning_content: msg.thinking_content,
        tags,
        is_finished: msg.is_finished,
        is_streaming: msg.is_streaming,
    })
}
```

**Modify `update_message` handler (line 88):**

Pass new fields to `db.update_message`:

```rust
let new_version = db.update_message(
    params.message_id,
    params.content.as_deref(),
    params.reasoning_content.as_deref(),
    params.tool_calls.as_deref(),
    params.is_finished,
    params.is_streaming,
)?;
```

---

### 8. `packages/rhd_chat_client/src/client.rs`

**Modify `add_message` method (line 471):**

The client method takes `AddMessageParams` which now includes the new fields — no signature change needed, but ensure the params struct is updated.

---

## Tests

### Unit Tests

1. **`test_message_with_streaming_flags`** in `rhd_chat_api::common::tests`:
   - Create a message with `is_finished: false, is_streaming: true`.
   - Serialize and verify JSON contains `"isFinished":false,"isStreaming":true`.
   - Deserialize and verify round-trip.

2. **`test_message_default_streaming_flags`** in `rhd_chat_api::common::tests`:
   - Deserialize a message JSON without `isFinished`/`isStreaming` fields.
   - Verify defaults: `is_finished: true, is_streaming: false`.

3. **`test_add_message_with_streaming`** in `rhd_db::chat_db::tests::message_tests`:
   - Add a message with `is_finished: false, is_streaming: true`.
   - Retrieve it and verify the flags.

4. **`test_update_message_streaming_flags`** in `rhd_db::chat_db::tests::message_tests`:
   - Add a message, then update `is_finished` to `true` and `is_streaming` to `false`.
   - Verify the update persisted.

---

## Implementation Notes

1. **Backward compatibility**: `is_finished` defaults to `true` and `is_streaming` defaults to `false`. Existing messages and API calls that don't specify these fields will behave exactly as before.

2. **SQLite boolean storage**: SQLite stores booleans as INTEGER (0/1). The `bool` type in rusqlite maps to INTEGER automatically.

3. **Migration safety**: The migration uses `ALTER TABLE ADD COLUMN` with `DEFAULT` values, which is safe for existing data — all existing rows get `is_finished=1, is_streaming=0`.

4. **`update_message` is now partial**: The old `update_message` only updated `content`. The new version updates only the fields that are `Some(...)`. This is a breaking change to the internal API but maintains backward compatibility at the WebSocket protocol level since `UpdateMessageParams` already had all-optional fields.
