# Chat Database Plan

## Goal

Add `ChatDb` to `rhd_db` crate for persisting chats and messages in `rhd_db/chats.db`.

## Scope

- New `ChatDb` struct with `Mutex<Connection>` (matches existing `ScenarioDb` pattern)
- Schema: `chats` and `messages` tables with foreign key cascade
- CRUD operations for chats and messages
- Truncation support for edit-and-resend flow
- Unit tests for all operations

## Schema

```sql
CREATE TABLE chats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);

CREATE INDEX idx_messages_chat_id ON messages(chat_id);
```

## Public API

```rust
pub struct ChatDb { conn: Mutex<Connection> }

pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

impl ChatDb {
    pub fn new(path: &str) -> DbResult<Self>;

    pub fn create_chat(&self, title: &str) -> DbResult<i64>;
    pub fn list_chats(&self) -> DbResult<Vec<ChatInfo>>;
    pub fn get_chat(&self, id: i64) -> DbResult<Option<ChatInfo>>;
    pub fn delete_chat(&self, id: i64) -> DbResult<()>;
    pub fn update_chat_title(&self, id: i64, title: &str) -> DbResult<()>;
    pub fn touch_chat(&self, id: i64) -> DbResult<()>; // update updated_at

    pub fn add_message(&self, chat_id: i64, role: &str, content: &str) -> DbResult<i64>;
    pub fn get_messages(&self, chat_id: i64) -> DbResult<Vec<Message>>;
    pub fn truncate_messages(&self, chat_id: i64, after_message_id: i64) -> DbResult<()>;
    pub fn update_message(&self, message_id: i64, content: &str) -> DbResult<()>;
}
```

## File Changes

| File | Change |
|------|--------|
| `packages/rhd_db/src/lib.rs` | Add `mod chat_db;` and re-export `ChatDb`, `ChatInfo`, `Message` |
| `packages/rhd_db/src/chat_db.rs` | **NEW** — `ChatDb` implementation |

## Implementation Steps

1. Create `chat_db.rs` with `ChatDb::new()` — open DB, enable WAL, enable foreign keys, create tables
2. Implement chat CRUD methods
3. Implement message CRUD methods
4. Implement `truncate_messages` — `DELETE FROM messages WHERE chat_id = ? AND id > ?`
5. Add unit tests covering:
   - Create/list/get/delete chat
   - Add/get messages
   - Truncate messages after specific id
   - Update message content
   - Cascade delete (deleting chat removes messages)
   - Persistence across reopens

## Design Notes

- Timestamps stored as ISO 8601 UTC strings (matches `ScenarioMeta` convention)
- `touch_chat()` updates `updated_at` — called when new message added or chat edited
- `truncate_messages` deletes all messages with `id > after_message_id` for given `chat_id` — used for edit-and-resend
- Foreign key `ON DELETE CASCADE` ensures message cleanup on chat delete
- Use `unchecked_transaction()` for atomicity where needed (matches `ScenarioDb::next_id`)

## Success Criteria

- [x] `ChatDb::new()` creates DB file and tables
- [x] Chat CRUD operations work correctly
- [x] Message CRUD operations work correctly
- [x] Truncation removes only messages after specified id
- [x] Cascade delete removes messages when chat deleted
- [x] All unit tests pass
- [x] WAL mode enabled

## Dependencies

None — standalone foundation. Other chat plans depend on this.
