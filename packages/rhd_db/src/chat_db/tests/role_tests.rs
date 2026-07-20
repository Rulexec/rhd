use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_set_and_get_active_role() {
    let path = "test_chat_active_role.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test").unwrap();

    // Initially no active role
    assert!(db.get_active_role(chat_id).unwrap().is_none());

    // Set active role
    db.set_active_role(chat_id, "project-a", "developer").unwrap();
    let role = db.get_active_role(chat_id).unwrap().unwrap();
    assert_eq!(role.0, "project-a");
    assert_eq!(role.1, "developer");

    // Clear active role
    db.clear_active_role(chat_id).unwrap();
    assert!(db.get_active_role(chat_id).unwrap().is_none());

    cleanup(path);
}

#[test]
fn test_roles_list_injected_flag() {
    let path = "test_chat_roles_injected.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test").unwrap();

    // Initially not injected
    assert!(!db.has_roles_list_been_injected(chat_id).unwrap());

    // Mark as injected
    db.mark_roles_list_injected(chat_id).unwrap();
    assert!(db.has_roles_list_been_injected(chat_id).unwrap());

    // Reset
    db.reset_roles_list_injected(chat_id).unwrap();
    assert!(!db.has_roles_list_been_injected(chat_id).unwrap());

    cleanup(path);
}

#[test]
fn test_migration_adds_role_columns() {
    let path = "test_chat_role_migration.db";
    cleanup(path);

    // Create database with old schema
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
                 active_model TEXT
             );",
        ).unwrap();
        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
    }

    // Open with ChatDb - should trigger migration
    let db = ChatDb::new(path).unwrap();

    // Verify new columns exist and work
    db.set_active_role(1, "proj", "role").unwrap();
    let role = db.get_active_role(1).unwrap().unwrap();
    assert_eq!(role.0, "proj");
    assert_eq!(role.1, "role");

    cleanup(path);
}

#[test]
fn test_role_prompt_pending_flag() {
    let path = "test_chat_role_prompt_pending.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test").unwrap();

    // Initially not pending
    assert!(!db.has_role_prompt_pending(chat_id).unwrap());

    // Set as pending
    db.set_role_prompt_pending(chat_id, true).unwrap();
    assert!(db.has_role_prompt_pending(chat_id).unwrap());

    // Clear pending
    db.set_role_prompt_pending(chat_id, false).unwrap();
    assert!(!db.has_role_prompt_pending(chat_id).unwrap());

    cleanup(path);
}
