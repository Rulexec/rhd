use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub active_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub model: Option<String>,
    pub thinking_content: Option<String>,
}

pub struct ChatDb {
    conn: Mutex<Connection>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

impl ChatDb {
    pub fn new(path: &str) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                active_model TEXT
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                model TEXT,
                FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);",
        )?;

        // Create chat_projects table if not exists
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chat_projects (
                chat_id INTEGER NOT NULL,
                project_name TEXT NOT NULL,
                system_prompt_added BOOLEAN NOT NULL DEFAULT 0,
                attached_at TEXT NOT NULL,
                PRIMARY KEY (chat_id, project_name),
                FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
            );",
        )?;

        // Migrate existing tables to add new columns
        self.migrate(&conn)?;

        Ok(())
    }

    fn migrate(&self, conn: &Connection) -> DbResult<()> {
        // Check if active_model column exists in chats table
        let has_active_model: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_model'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_active_model {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN active_model TEXT")?;
        }

        // Check if model column exists in messages table
        let has_model: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='model'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_model {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN model TEXT")?;
        }

        // Check if thinking_content column exists in messages table
        let has_thinking_content: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='thinking_content'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_thinking_content {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN thinking_content TEXT")?;
        }

        Ok(())
    }

    pub fn create_chat(&self, title: &str) -> DbResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES (?1, ?2, ?3)",
            params![title, now, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_chats(&self) -> DbResult<Vec<ChatInfo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, active_model FROM chats ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                active_model: row.get(4)?,
            })
        })?;
        let mut chats = Vec::new();
        for row in rows {
            chats.push(row?);
        }
        Ok(chats)
    }

    pub fn get_chat(&self, id: i64) -> DbResult<Option<ChatInfo>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, active_model FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                active_model: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn delete_chat(&self, id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_all_chats(&self) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute("DELETE FROM chats", [])?;
        Ok(())
    }

    pub fn update_chat_title(&self, id: i64, title: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;
        Ok(())
    }

    pub fn update_chat_active_model(&self, id: i64, model: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET active_model = ?1, updated_at = ?2 WHERE id = ?3",
            params![model, now, id],
        )?;
        Ok(())
    }

    pub fn touch_chat(&self, id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn add_message(&self, chat_id: i64, role: &str, content: &str, model: Option<&str>, thinking_content: Option<&str>) -> DbResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        let now = now_iso();
        tx.execute(
            "INSERT INTO messages (chat_id, role, content, created_at, model, thinking_content) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chat_id, role, content, now, model, thinking_content],
        )?;
        let message_id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
            params![now, chat_id],
        )?;
        tx.commit()?;
        Ok(message_id)
    }

    pub fn get_messages(&self, chat_id: i64) -> DbResult<Vec<Message>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, role, content, created_at, model, thinking_content FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                model: row.get(5)?,
                thinking_content: row.get(6)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn truncate_messages(&self, chat_id: i64, after_message_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "DELETE FROM messages WHERE chat_id = ?1 AND id > ?2",
            params![chat_id, after_message_id],
        )?;
        Ok(())
    }

    pub fn get_message(&self, message_id: i64) -> DbResult<Option<Message>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, role, content, created_at, model, thinking_content FROM messages WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![message_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                model: row.get(5)?,
                thinking_content: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_message(&self, message_id: i64, content: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        let chat_id: i64 = tx.query_row(
            "SELECT chat_id FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE messages SET content = ?1 WHERE id = ?2",
            params![content, message_id],
        )?;
        let now = now_iso();
        tx.execute(
            "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
            params![now, chat_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn attach_project(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "INSERT OR IGNORE INTO chat_projects (chat_id, project_name, system_prompt_added, attached_at) VALUES (?1, ?2, 0, ?3)",
            params![chat_id, project_name, now],
        )?;
        Ok(())
    }

    pub fn detach_project(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "DELETE FROM chat_projects WHERE chat_id = ?1 AND project_name = ?2",
            params![chat_id, project_name],
        )?;
        Ok(())
    }

    pub fn get_chat_projects(&self, chat_id: i64) -> DbResult<Vec<(String, bool)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT project_name, system_prompt_added FROM chat_projects WHERE chat_id = ?1 ORDER BY attached_at ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn mark_system_prompt_added(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chat_projects SET system_prompt_added = 1 WHERE chat_id = ?1 AND project_name = ?2",
            params![chat_id, project_name],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
