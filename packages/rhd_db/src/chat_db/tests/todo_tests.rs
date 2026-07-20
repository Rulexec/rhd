use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_set_and_get_todo_list() {
    let path = "test_chat_todo_list.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test").unwrap();

    assert!(db.get_todo_list(chat_id).unwrap().is_none());

    let todo_list = "[x] Task 1\n[-] Task 2\n[ ] Task 3";
    db.set_todo_list(chat_id, todo_list).unwrap();
    
    let retrieved = db.get_todo_list(chat_id).unwrap().unwrap();
    assert_eq!(retrieved, todo_list);

    let new_todo_list = "[x] Task 1\n[x] Task 2\n[-] Task 3";
    db.set_todo_list(chat_id, new_todo_list).unwrap();
    
    let retrieved = db.get_todo_list(chat_id).unwrap().unwrap();
    assert_eq!(retrieved, new_todo_list);

    cleanup(path);
}

#[test]
fn test_migration_adds_todo_list_column() {
    let path = "test_chat_todo_migration.db";
    cleanup(path);

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
                 active_role_project TEXT,
                 active_role_name TEXT,
                 roles_list_injected BOOLEAN NOT NULL DEFAULT 0,
                 role_prompt_pending BOOLEAN NOT NULL DEFAULT 0
             );",
        ).unwrap();
        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
    }

    let db = ChatDb::new(path).unwrap();

    db.set_todo_list(1, "[x] Test task").unwrap();
    let todo = db.get_todo_list(1).unwrap().unwrap();
    assert_eq!(todo, "[x] Test task");

    cleanup(path);
}
