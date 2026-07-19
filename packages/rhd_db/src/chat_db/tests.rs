use super::*;
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path));
    let _ = fs::remove_file(format!("{}-shm", path));
}

#[test]
fn test_create_and_list_chats() {
    let path = "test_chat_create.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id1 = db.create_chat("First").unwrap();
    let id2 = db.create_chat("Second").unwrap();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let chats = db.list_chats().unwrap();
    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0].title, "Second");
    assert_eq!(chats[1].title, "First");

    cleanup(path);
}

#[test]
fn test_get_chat() {
    let path = "test_chat_get.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.id, id);
    assert_eq!(chat.title, "Test");
    assert_eq!(chat.created_at, chat.updated_at);

    assert!(db.get_chat(999).unwrap().is_none());

    cleanup(path);
}

#[test]
fn test_delete_chat() {
    let path = "test_chat_delete.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("ToDelete").unwrap();
    assert!(db.get_chat(id).unwrap().is_some());

    db.delete_chat(id).unwrap();
    assert!(db.get_chat(id).unwrap().is_none());
    assert!(db.list_chats().unwrap().is_empty());

    cleanup(path);
}

#[test]
fn test_delete_all_chats() {
    let path = "test_chat_delete_all.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id1 = db.create_chat("First").unwrap();
    let id2 = db.create_chat("Second").unwrap();
    assert_eq!(db.list_chats().unwrap().len(), 2);

    db.delete_all_chats().unwrap();
    assert!(db.list_chats().unwrap().is_empty());
    assert!(db.get_chat(id1).unwrap().is_none());
    assert!(db.get_chat(id2).unwrap().is_none());

    cleanup(path);
}

#[test]
fn test_update_chat_title() {
    let path = "test_chat_title.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Old").unwrap();

    db.update_chat_title(id, "New").unwrap();
    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.title, "New");

    cleanup(path);
}

#[test]
fn test_add_and_get_messages() {
    let path = "test_chat_messages.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let msg1 = db.add_message(chat_id, "user", "Hello", Some("gpt4"), None).unwrap();
    let msg2 = db.add_message(chat_id, "assistant", "Hi there", Some("gpt4"), None).unwrap();
    assert_eq!(msg1, 1);
    assert_eq!(msg2, 2);

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[0].model, Some("gpt4".to_string()));
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi there");
    assert_eq!(messages[1].model, Some("gpt4".to_string()));

    cleanup(path);
}

#[test]
fn test_truncate_messages() {
    let path = "test_chat_truncate.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let msg1 = db.add_message(chat_id, "user", "First", None, None).unwrap();
    let _msg2 = db.add_message(chat_id, "assistant", "Second", None, None).unwrap();
    let _msg3 = db.add_message(chat_id, "user", "Third", None, None).unwrap();

    db.truncate_messages(chat_id, msg1).unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "First");

    cleanup(path);
}

#[test]
fn test_update_message() {
    let path = "test_chat_update_msg.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let msg_id = db.add_message(chat_id, "user", "Original", None, None).unwrap();

    db.update_message(msg_id, "Updated").unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.content, "Updated");

    cleanup(path);
}

#[test]
fn test_update_chat_active_model() {
    let path = "test_chat_active_model.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.active_model, None);

    db.update_chat_active_model(id, "gpt4").unwrap();
    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.active_model, Some("gpt4".to_string()));

    cleanup(path);
}

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

#[test]
fn test_attach_and_get_projects() {
    let path = "test_chat_projects.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-b").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].0, "project-a");
    assert_eq!(projects[0].1, false);
    assert_eq!(projects[1].0, "project-b");
    assert_eq!(projects[1].1, false);

    cleanup(path);
}

#[test]
fn test_detach_project() {
    let path = "test_chat_projects_detach.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-b").unwrap();
    db.detach_project(chat_id, "project-a").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].0, "project-b");

    cleanup(path);
}

#[test]
fn test_mark_system_prompt_added() {
    let path = "test_chat_projects_prompt.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects[0].1, false);

    db.mark_system_prompt_added(chat_id, "project-a").unwrap();
    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects[0].1, true);

    cleanup(path);
}

#[test]
fn test_attach_project_idempotent() {
    let path = "test_chat_projects_idempotent.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-a").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 1);

    cleanup(path);
}

#[test]
fn test_cascade_delete_projects() {
    let path = "test_chat_projects_cascade.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.delete_chat(chat_id).unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 0);

    cleanup(path);
}

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
