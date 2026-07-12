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
    pub active_role_project: Option<String>,
    pub active_role_name: Option<String>,
    pub todo_list: Option<String>,
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

        // Check if active_role_project column exists in chats table
        let has_active_role_project: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_project'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_active_role_project {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_project TEXT")?;
        }

        // Check if active_role_name column exists in chats table
        let has_active_role_name: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_name'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_active_role_name {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_name TEXT")?;
        }

        // Check if roles_list_injected column exists in chats table
        let has_roles_list_injected: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='roles_list_injected'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_roles_list_injected {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN roles_list_injected BOOLEAN NOT NULL DEFAULT 0")?;
        }

        // Check if role_prompt_pending column exists in chats table
        let has_role_prompt_pending: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='role_prompt_pending'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_role_prompt_pending {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN role_prompt_pending BOOLEAN NOT NULL DEFAULT 0")?;
        }

        // Check if todo_list column exists in chats table
        let has_todo_list: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='todo_list'")?
            .query_row([], |row| row.get::<_, i64>(0))?
            > 0;

        if !has_todo_list {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN todo_list TEXT")?;
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
            "SELECT id, title, created_at, updated_at, active_model, active_role_project, active_role_name, todo_list FROM chats ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                active_model: row.get(4)?,
                active_role_project: row.get(5)?,
                active_role_name: row.get(6)?,
                todo_list: row.get(7)?,
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
            "SELECT id, title, created_at, updated_at, active_model, active_role_project, active_role_name, todo_list FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                active_model: row.get(4)?,
                active_role_project: row.get(5)?,
                active_role_name: row.get(6)?,
                todo_list: row.get(7)?,
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

    /// Sets the active role for a chat
    pub fn set_active_role(&self, chat_id: i64, project_name: &str, role_name: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET active_role_project = ?1, active_role_name = ?2, updated_at = ?3 WHERE id = ?4",
            params![project_name, role_name, now, chat_id],
        )?;
        Ok(())
    }

    /// Clears the active role for a chat (when no role is selected)
    pub fn clear_active_role(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET active_role_project = NULL, active_role_name = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, chat_id],
        )?;
        Ok(())
    }

    /// Gets the active role for a chat, returns (project_name, role_name) or None
    pub fn get_active_role(&self, chat_id: i64) -> DbResult<Option<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT active_role_project, active_role_name FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![chat_id], |row| {
            let project: Option<String> = row.get(0)?;
            let role: Option<String> = row.get(1)?;
            Ok((project, role))
        })?;
        match rows.next() {
            Some(row) => {
                let (project, role) = row?;
                match (project, role) {
                    (Some(p), Some(r)) => Ok(Some((p, r))),
                    _ => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    /// Marks that the roles list prompt has been injected for this chat
    pub fn mark_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chats SET roles_list_injected = 1 WHERE id = ?1",
            params![chat_id],
        )?;
        Ok(())
    }

    /// Checks if the roles list prompt has been injected for this chat
    pub fn has_roles_list_been_injected(&self, chat_id: i64) -> DbResult<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let injected: bool = conn
            .prepare("SELECT roles_list_injected FROM chats WHERE id = ?1")?
            .query_row(params![chat_id], |row| row.get(0))?;
        Ok(injected)
    }

    /// Resets the roles list injection flag (called when new project with roles is attached)
    pub fn reset_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chats SET roles_list_injected = 0 WHERE id = ?1",
            params![chat_id],
        )?;
        Ok(())
    }

    /// Sets the role prompt pending flag (called when role is changed)
    pub fn set_role_prompt_pending(&self, chat_id: i64, pending: bool) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chats SET role_prompt_pending = ?1 WHERE id = ?2",
            params![pending, chat_id],
        )?;
        Ok(())
    }

    /// Checks if the role prompt is pending injection
    pub fn has_role_prompt_pending(&self, chat_id: i64) -> DbResult<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let pending: bool = conn
            .prepare("SELECT role_prompt_pending FROM chats WHERE id = ?1")?
            .query_row(params![chat_id], |row| row.get(0))?;
        Ok(pending)
    }

    /// Sets the todo list for a chat
    pub fn set_todo_list(&self, chat_id: i64, todo_list: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET todo_list = ?1, updated_at = ?2 WHERE id = ?3",
            params![todo_list, now, chat_id],
        )?;
        Ok(())
    }

    /// Gets the todo list for a chat, returns None if not set
    pub fn get_todo_list(&self, chat_id: i64) -> DbResult<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT todo_list FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![chat_id], |row| {
            row.get::<_, Option<String>>(0)
        })?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Ok(None),
        }
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
}
