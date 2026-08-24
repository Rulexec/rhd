use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::{Message, ToolCall};
use crate::{DbError, DbResult};

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn add_queue_message(
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
        "INSERT INTO messages_queue (chat_id, role, content, created_at, model, thinking_content) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
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

pub(crate) fn get_queue_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Vec<Message>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls FROM messages_queue WHERE chat_id = ?1 ORDER BY id ASC",
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
            is_finished: true,
            is_streaming: false,
        })
    })?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

pub(crate) fn get_queue_message(conn: &Mutex<Connection>, message_id: i64) -> DbResult<Option<Message>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls FROM messages_queue WHERE id = ?1",
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
            is_finished: true,
            is_streaming: false,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub(crate) fn update_queue_message(
    conn: &Mutex<Connection>,
    message_id: i64,
    content: &str,
) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages_queue WHERE id = ?1",
        params![message_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "UPDATE messages_queue SET content = ?1 WHERE id = ?2",
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

/// Insert a queue message with an explicit ID
pub(crate) fn insert_queue_message(
    conn: &Mutex<Connection>,
    message: &Message,
) -> DbResult<(Message, i64)> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let tool_calls_json = message
        .tool_calls
        .as_ref()
        .map(|tc| serde_json::to_string(tc).unwrap_or_default());
    tx.execute(
        "INSERT INTO messages_queue (id, chat_id, role, content, created_at, model, thinking_content, tool_calls)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            message.id,
            message.chat_id,
            message.role,
            message.content,
            message.created_at,
            message.model,
            message.thinking_content,
            tool_calls_json,
        ],
    )?;
    let now = now_iso();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, message.chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![message.chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok((message.clone(), new_version))
}

/// Update a full queue message
pub(crate) fn update_queue_message_full(
    conn: &Mutex<Connection>,
    message: &Message,
) -> DbResult<(Message, i64)> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let tool_calls_json = message
        .tool_calls
        .as_ref()
        .map(|tc| serde_json::to_string(tc).unwrap_or_default());
    tx.execute(
        "UPDATE messages_queue SET role = ?1, content = ?2, thinking_content = ?3, tool_calls = ?4, model = ?5 WHERE id = ?6",
        params![
            message.role,
            message.content,
            message.thinking_content,
            tool_calls_json,
            message.model,
            message.id,
        ],
    )?;
    let now = now_iso();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, message.chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        params![message.chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok((message.clone(), new_version))
}

/// Delete a queue message by ID
pub(crate) fn delete_queue_message(conn: &Mutex<Connection>, message_id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages_queue WHERE id = ?1",
        params![message_id],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM messages_queue WHERE id = ?1", params![message_id])?;
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

/// Delete all queue messages for a chat
pub(crate) fn delete_all_queue_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM messages_queue WHERE chat_id = ?1", params![chat_id])?;
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

/// Count queue messages for a chat
pub(crate) fn count_queue_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages_queue WHERE chat_id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    Ok(count)
}
