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
        .add_queue_message(1, None, "tool", "queued result", None, None, Some("call_2"))
        .unwrap();
    assert_eq!(
        db.get_queue_messages(1).unwrap()[0].tool_call_id,
        Some("call_2".to_string())
    );

    cleanup(path);
}

#[test]
fn test_migration_backfills_queue_position() {
    let path = "test_chat_migration_queue_position.db";
    cleanup(path);

    // Create database with the pre-position messages_queue schema
    // (tool_call_id present, position absent).
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

             CREATE TABLE messages_queue (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 chat_id INTEGER NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 model TEXT,
                 thinking_content TEXT,
                 tool_calls TEXT,
                 tool_call_id TEXT,
                 FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
             );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // Pre-existing queued rows with ids that do not start at 1.
        for (id, content) in [(10i64, "first"), (11, "second"), (12, "third")] {
            conn.execute(
                "INSERT INTO messages_queue (id, chat_id, role, content, created_at) VALUES (?1, 1, 'user', ?2, '2024-01-01T00:00:00Z')",
                (id, content),
            )
            .unwrap();
        }
    }

    // Opening with ChatDb must add the position column and backfill it.
    let db = ChatDb::new(path).unwrap();

    // The effective order (previously by id) is unchanged.
    let messages = db.get_queue_messages(1).unwrap();
    assert_eq!(
        messages.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![10, 11, 12]
    );

    // Migration backfilled position = id for the pre-existing rows.
    let positions: Vec<(i64, i64)> = {
        let conn = db.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, position FROM messages_queue ORDER BY position ASC, id ASC")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect()
    };
    assert_eq!(positions, vec![(10, 10), (11, 11), (12, 12)]);

    // New appends continue past the backfilled maximum.
    let (new_id, _) = db
        .add_queue_message(1, None, "user", "fourth", None, None, None)
        .unwrap();
    let messages = db.get_queue_messages(1).unwrap();
    assert_eq!(messages.last().unwrap().id, new_id);
    assert_eq!(new_id, 13);

    cleanup(path);
}
