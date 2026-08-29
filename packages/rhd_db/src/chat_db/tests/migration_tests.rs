use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_migration_from_old_schema() {
    let path = "test_chat_migration.db";
    cleanup(path);

    // Create database with old schema (without active_model and model columns)
    {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;

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

             CREATE INDEX idx_messages_chat_id ON messages(chat_id);",
        ).unwrap();

        // Insert some test data
        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages (chat_id, role, content, created_at) VALUES (1, 'user', 'Old message', '2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
    }

    // Now open with ChatDb which should trigger migration
    let db = ChatDb::new(path).unwrap();

    // Verify we can read the old data
    let chats = db.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].title, "Old Chat");
    assert_eq!(chats[0].active_model, None);

    let messages = db.get_messages(1).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Old message");
    assert_eq!(messages[0].model, None);

    // Verify we can use the new columns
    let (msg_id, _version) = db.add_message(1, "assistant", "New message", Some("gpt4"), None, true, false, None).unwrap();
    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.model, Some("gpt4".to_string()));

    cleanup(path);
}

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
