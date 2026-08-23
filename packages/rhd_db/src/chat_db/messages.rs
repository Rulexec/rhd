use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::{Message, ToolCall};
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
) -> DbResult<(i64, i64)> {
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
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok((message_id, new_version))
}

pub(crate) fn get_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Vec<Message>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![chat_id], |row| {
        let tool_calls_json: Option<String> = row.get(7)?;
        let tool_calls = tool_calls_json
            .and_then(|json| serde_json::from_str::<Vec<ToolCall>>(&json).ok());
        Ok(Message {
            id: row.get(0)?,
            chat_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            model: row.get(5)?,
            thinking_content: row.get(6)?,
            tool_calls,
        })
    })?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub(crate) fn get_message(conn: &Mutex<Connection>, message_id: i64) -> DbResult<Option<Message>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls FROM messages WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![message_id], |row| {
        let tool_calls_json: Option<String> = row.get(7)?;
        let tool_calls = tool_calls_json
            .and_then(|json| serde_json::from_str::<Vec<ToolCall>>(&json).ok());
        Ok(Message {
            id: row.get(0)?,
            chat_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            model: row.get(5)?,
            thinking_content: row.get(6)?,
            tool_calls,
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
) -> DbResult<i64> {
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
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Delete a message by ID
pub(crate) fn delete_message(conn: &Mutex<Connection>, message_id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages WHERE id = ?1",
        params![message_id],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM messages WHERE id = ?1", params![message_id])?;
    let now = now_iso();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

