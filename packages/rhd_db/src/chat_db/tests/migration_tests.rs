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
    db.update_chat_active_model(1, "gpt4").unwrap();
    let chat = db.get_chat(1).unwrap().unwrap();
    assert_eq!(chat.active_model, Some("gpt4".to_string()));

    let msg_id = db.add_message(1, "assistant", "New message", Some("gpt4"), None).unwrap();
    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.model, Some("gpt4".to_string()));

    cleanup(path);
}
