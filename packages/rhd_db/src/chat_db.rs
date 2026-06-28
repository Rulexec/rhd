use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
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
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);",
        )?;

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
            "SELECT id, title, created_at, updated_at FROM chats ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
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
            "SELECT id, title, created_at, updated_at FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(ChatInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
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

    pub fn add_message(&self, chat_id: i64, role: &str, content: &str) -> DbResult<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        let now = now_iso();
        tx.execute(
            "INSERT INTO messages (chat_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![chat_id, role, content, now],
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
            "SELECT id, chat_id, role, content, created_at FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
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
            "SELECT id, chat_id, role, content, created_at FROM messages WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![message_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
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

        let msg1 = db.add_message(chat_id, "user", "Hello").unwrap();
        let msg2 = db.add_message(chat_id, "assistant", "Hi there").unwrap();
        assert_eq!(msg1, 1);
        assert_eq!(msg2, 2);

        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hi there");

        cleanup(path);
    }

    #[test]
    fn test_truncate_messages() {
        let path = "test_chat_truncate.db";
        cleanup(path);

        let db = ChatDb::new(path).unwrap();
        let chat_id = db.create_chat("Chat").unwrap();

        let msg1 = db.add_message(chat_id, "user", "First").unwrap();
        let _msg2 = db.add_message(chat_id, "assistant", "Second").unwrap();
        let _msg3 = db.add_message(chat_id, "user", "Third").unwrap();

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
        let msg_id = db.add_message(chat_id, "user", "Original").unwrap();

        db.update_message(msg_id, "Edited").unwrap();

        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages[0].content, "Edited");

        cleanup(path);
    }

    #[test]
    fn test_cascade_delete() {
        let path = "test_chat_cascade.db";
        cleanup(path);

        let db = ChatDb::new(path).unwrap();
        let chat_id = db.create_chat("Chat").unwrap();
        db.add_message(chat_id, "user", "Msg1").unwrap();
        db.add_message(chat_id, "assistant", "Msg2").unwrap();

        db.delete_chat(chat_id).unwrap();

        let messages = db.get_messages(chat_id).unwrap();
        assert!(messages.is_empty());

        cleanup(path);
    }

    #[test]
    fn test_persistence_across_reopens() {
        let path = "test_chat_persist.db";
        cleanup(path);

        let chat_id;
        {
            let db = ChatDb::new(path).unwrap();
            chat_id = db.create_chat("Persistent").unwrap();
            db.add_message(chat_id, "user", "Hello").unwrap();
        }

        {
            let db = ChatDb::new(path).unwrap();
            let chat = db.get_chat(chat_id).unwrap().unwrap();
            assert_eq!(chat.title, "Persistent");

            let messages = db.get_messages(chat_id).unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].content, "Hello");
        }

        cleanup(path);
    }

    #[test]
    fn test_add_message_touches_chat() {
        let path = "test_chat_touch.db";
        cleanup(path);

        let db = ChatDb::new(path).unwrap();
        let chat_id = db.create_chat("Chat").unwrap();
        let created = db.get_chat(chat_id).unwrap().unwrap();

        db.add_message(chat_id, "user", "Hi").unwrap();
        let updated = db.get_chat(chat_id).unwrap().unwrap();

        assert!(updated.updated_at >= created.updated_at);

        cleanup(path);
    }
}
