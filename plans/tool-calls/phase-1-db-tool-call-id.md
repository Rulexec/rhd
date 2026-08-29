# Phase 1: Storage — `tool_call_id` Column + DB Plumbing (milestone P1)

## Overview

Add a first-class nullable `tool_call_id` column to the `messages` and `messages_queue`
tables and thread it through the `rhd_db` crate: the `Message` struct, the add/insert
functions, both SELECT lists, the row mappings, and the `ChatDb` wrapper methods.

This implements decision **D1** of the milestone plan: `tool_call_id` is a real column,
not a tag or a blob trick, so a `tool`-role message can eventually carry the id of the
assistant tool call it answers. `tool_call_id` is **immutable after creation** — it is
deliberately *not* added to `update_message` / `update_queue_message_full`.

**Scope:** `packages/rhd_db` only, plus mechanical `, None` call-site updates in
`rhd_chat_server` and `rhd_db` tests so the workspace keeps compiling after this phase.

**Out of scope:** API types (phase-2), server validation (phase-2), plugin logic (phases 4–6).

## Dependencies

- None — this is the first phase.
- **Must complete before** phase-2 (server handlers pass the value to these DB functions).

## Files to Modify

### 1. `packages/rhd_db/src/chat_db/schema.rs`

**Modify `migrate()`** — append two guarded blocks at the end of the function, after the
existing `is_streaming` block (around line 226), following the established
`pragma_table_info` guard pattern:

```rust
    // Check if tool_call_id column exists in messages table
    let has_tool_call_id: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='tool_call_id'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_tool_call_id {
        conn.execute_batch("ALTER TABLE messages ADD COLUMN tool_call_id TEXT")?;
    }

    // Check if tool_call_id column exists in messages_queue table
    let has_queue_tool_call_id: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('messages_queue') WHERE name='tool_call_id'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_queue_tool_call_id {
        conn.execute_batch("ALTER TABLE messages_queue ADD COLUMN tool_call_id TEXT")?;
    }
```

Do **not** touch the `CREATE TABLE` statements — every added column in this codebase
(`thinking_content`, `tool_calls`, `is_finished`, `is_streaming`) is delivered purely via
`migrate()`, which also covers freshly created DBs.

### 2. `packages/rhd_db/src/chat_db/mod.rs`

**Modify `Message`** (line 35) — add the field after `content` so it groups with `role`:

```rust
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    /// For `tool`-role messages: the id of the assistant tool call this message answers.
    pub tool_call_id: Option<String>,
    pub created_at: String,
    pub model: Option<String>,
    pub thinking_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub is_finished: bool,
    pub is_streaming: bool,
}
```

**Modify `ChatDb::add_message`** (line 118) — append `tool_call_id: Option<&str>` as the
**last** parameter and forward it:

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
        tool_call_id: Option<&str>,
    ) -> DbResult<(i64, i64)> {
        messages::add_message(&self.conn, chat_id, role, content, model, thinking_content, is_finished, is_streaming, tool_call_id)
    }
```

**Modify `ChatDb::add_queue_message`** (line 247) — same pattern, append the parameter:

```rust
    pub fn add_queue_message(
        &self,
        chat_id: i64,
        role: &str,
        content: &str,
        model: Option<&str>,
        thinking_content: Option<&str>,
        tool_call_id: Option<&str>,
    ) -> DbResult<(i64, i64)> {
        messages_queue::add_queue_message(&self.conn, chat_id, role, content, model, thinking_content, tool_call_id)
    }
```

> Appending new parameters at the end (instead of inserting mid-list) keeps every existing
> call site a one-token change (`, None`) while the compiler still catches mistakes —
> all parameters are typed.

### 3. `packages/rhd_db/src/chat_db/messages.rs`

**Modify `add_message`** (line 12): append `tool_call_id: Option<&str>` parameter; extend
the INSERT (column list + `?9`):

```rust
    tx.execute(
        "INSERT INTO messages (chat_id, role, content, created_at, model, thinking_content, is_finished, is_streaming, tool_call_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![chat_id, role, content, now, model, thinking_content, is_finished, is_streaming, tool_call_id],
    )?;
```

**Modify `get_messages`** (line 45) and **`get_message`** (line 76): append `tool_call_id`
as the **last** column in each SELECT (so indices 0–9 stay unchanged) and read it at the
new index 10 in both row-closure `Message { ... }` literals:

```rust
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls, is_finished, is_streaming, tool_call_id FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
```
```rust
            tool_call_id: row.get(10)?,
```
(and the same column appended to the `WHERE id = ?1` SELECT in `get_message`)

**Leave `update_message` and `update_message_tool_call_tags` untouched** — `tool_call_id`
is immutable after creation (D1).

### 4. `packages/rhd_db/src/chat_db/messages_queue.rs`

**Modify `add_queue_message`** (line 12): append `tool_call_id: Option<&str>`; extend the
INSERT to `?7`:

```rust
    tx.execute(
        "INSERT INTO messages_queue (chat_id, role, content, created_at, model, thinking_content, tool_call_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![chat_id, role, content, now, model, thinking_content, tool_call_id],
    )?;
```

**Modify `get_queue_messages`** (line 43) and **`get_queue_message`** (line 74): append
`tool_call_id` as the last SELECT column (new index 8) and add
`tool_call_id: row.get(8)?` to both `Message { ... }` literals.

**Modify `insert_queue_message`** (line 137): append `tool_call_id` to the explicit column
list and `?9` to the VALUES, passing `message.tool_call_id`:

```rust
    tx.execute(
        "INSERT INTO messages_queue (id, chat_id, role, content, created_at, model, thinking_content, tool_calls, tool_call_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            message.id,
            message.chat_id,
            message.role,
            message.content,
            message.created_at,
            message.model,
            message.thinking_content,
            tool_calls_json,
            message.tool_call_id,
        ],
    )?;
```

**Leave `update_queue_message_full` untouched** — immutability of `tool_call_id` (D1).

### 5. Compile-fix call sites (mechanical `, None` / `None` additions)

The workspace must compile after this phase. Update every caller the compiler surfaces:

- `packages/rhd_chat_server/src/handlers/message.rs:70` — append `None,` to the
  `db.add_message(...)` call (phase-2 replaces this with the real pass-through).
- `packages/rhd_chat_server/src/handlers/queue_message.rs:75` — append `None,` to the
  `db.add_queue_message(...)` call (phase-2 replaces it).
- `packages/rhd_db/src/chat_db/tests/message_tests.rs` lines 13, 14, 37, 49, 131 — append
  `None` to each `db.add_message(...)` call.
- `packages/rhd_db/src/chat_db/tests/migration_tests.rs:61` — append `None`.
- `packages/rhd_db/src/chat_db/tests/tags_plugins_events_tests.rs:37` — append `None`.

## Tests

### `packages/rhd_db/src/chat_db/tests/message_tests.rs` — add

```rust
#[test]
fn test_tool_message_round_trip_with_tool_call_id() {
    let path = "test_chat_tool_call_id.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg_id, _) = db
        .add_message(chat_id, "tool", "Sunny, 22C", None, None, true, false, Some("call_abc"))
        .unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.role, "tool");
    assert_eq!(msg.tool_call_id, Some("call_abc".to_string()));

    // Also visible through the list reader.
    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages[0].tool_call_id, Some("call_abc".to_string()));

    cleanup(path);
}

#[test]
fn test_tool_call_id_none_stays_none() {
    let path = "test_chat_tool_call_id_none.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let (msg_id, _) = db
        .add_message(chat_id, "user", "Hello", None, None, true, false, None)
        .unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.tool_call_id, None);

    cleanup(path);
}

#[test]
fn test_queue_message_round_trip_with_tool_call_id() {
    let path = "test_queue_tool_call_id.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg_id, _) = db
        .add_queue_message(chat_id, "tool", "result", None, None, Some("call_xyz"))
        .unwrap();

    let msg = db.get_queue_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.tool_call_id, Some("call_xyz".to_string()));

    let messages = db.get_queue_messages(chat_id).unwrap();
    assert_eq!(messages[0].tool_call_id, Some("call_xyz".to_string()));

    cleanup(path);
}
```

### `packages/rhd_db/src/chat_db/tests/migration_tests.rs` — add

A test proving an existing DB without the column is migrated (mirrors
`test_migration_from_old_schema`):

```rust
#[test]
fn test_migration_adds_tool_call_id_columns() {
    let path = "test_chat_migration_tool_call_id.db";
    cleanup(path);

    // Create database with the pre-tool_call_id schema (messages_queue without the column).
    {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;

             CREATE TABLE chats (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 active_model TEXT,
                 version INTEGER NOT NULL DEFAULT 1
             );

             CREATE TABLE messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 chat_id INTEGER NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 model TEXT,
                 thinking_content TEXT,
                 tool_calls TEXT,
                 is_finished INTEGER NOT NULL DEFAULT 1,
                 is_streaming INTEGER NOT NULL DEFAULT 0,
                 FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
             );

             CREATE TABLE messages_queue (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 chat_id INTEGER NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 model TEXT,
                 thinking_content TEXT,
                 tool_calls TEXT,
                 FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
             );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (chat_id, role, content, created_at) VALUES (1, 'user', 'Old message', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    // Opening with ChatDb must add the columns.
    let db = ChatDb::new(path).unwrap();

    // Old rows read back with tool_call_id = None.
    let messages = db.get_messages(1).unwrap();
    assert_eq!(messages[0].tool_call_id, None);

    // New rows can carry an id.
    let (msg_id, _) = db
        .add_message(1, "tool", "result", None, None, true, false, Some("call_1"))
        .unwrap();
    assert_eq!(
        db.get_message(msg_id).unwrap().unwrap().tool_call_id,
        Some("call_1".to_string())
    );

    let (_, _) = db
        .add_queue_message(1, "tool", "queued result", None, None, Some("call_2"))
        .unwrap();
    assert_eq!(
        db.get_queue_messages(1).unwrap()[0].tool_call_id,
        Some("call_2".to_string())
    );

    cleanup(path);
}
```

## Implementation Notes

1. **Nullable TEXT, no NOT NULL constraint** — only `tool` messages use the column; user /
   assistant / system rows keep `NULL`. The server-level "role tool requires toolCallId"
   validation lands in phase-2, not here: the DB layer stays permissive.
2. **Append columns at the end of SELECT lists** — keeps existing `row.get(N)` indices
   stable and makes the diff minimal; the new index is the last one.
3. **Both tables get the column** (D1): `rhd_db::Message` is shared by queue readers, so a
   queued tool message keeps its id and can later pass the D4 integrity check when promoted.
4. **Immutability**: do not add `tool_call_id` to `update_message`,
   `update_queue_message`, or `update_queue_message_full`. The only write paths are the two
   INSERTs and `insert_queue_message` (which inserts a full row, including the id).
5. **`insert_queue_message` / `update_queue_message_full`** are used by queue promotion
   flows; the former must persist the id so a promoted message doesn't lose it.

## Validation

```bash
mise run check-cargo   # workspace compiles (server handlers fixed with None)
mise run test-cargo    # new rhd_db tests pass
```
