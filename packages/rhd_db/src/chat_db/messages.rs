use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::Message;
use crate::{DbError, DbResult};

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn add_message(
    conn: &Mutex<Connection>,
    chat_id: i64,
    role: &str,
    content: &str,
    model: Option<&str>,
    thinking_content: Option<&str>,
) -> DbResult<i64> {
    let conn = conn
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

pub(crate) fn get_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Vec<Message>> {
    let conn = conn
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

pub(crate) fn truncate_messages(
    conn: &Mutex<Connection>,
    chat_id: i64,
    after_message_id: i64,
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "DELETE FROM messages WHERE chat_id = ?1 AND id > ?2",
        params![chat_id, after_message_id],
    )?;
    Ok(())
}

pub(crate) fn get_message(conn: &Mutex<Connection>, message_id: i64) -> DbResult<Option<Message>> {
    let conn = conn
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

pub(crate) fn update_message(
    conn: &Mutex<Connection>,
    message_id: i64,
    content: &str,
) -> DbResult<()> {
    let conn = conn
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
