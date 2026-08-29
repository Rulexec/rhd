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
    is_finished: bool,
    is_streaming: bool,
    tool_call_id: Option<&str>,
) -> DbResult<(i64, i64)> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let now = now_iso();
    tx.execute(
        "INSERT INTO messages (chat_id, role, content, created_at, model, thinking_content, is_finished, is_streaming, tool_call_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![chat_id, role, content, now, model, thinking_content, is_finished, is_streaming, tool_call_id],
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
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls, is_finished, is_streaming, tool_call_id FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
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
            is_finished: row.get::<_, bool>(8)?,
            is_streaming: row.get::<_, bool>(9)?,
            tool_call_id: row.get(10)?,
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
        "SELECT id, chat_id, role, content, created_at, model, thinking_content, tool_calls, is_finished, is_streaming, tool_call_id FROM messages WHERE id = ?1",
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
            is_finished: row.get::<_, bool>(8)?,
            is_streaming: row.get::<_, bool>(9)?,
            tool_call_id: row.get(10)?,
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
    content: Option<&str>,
    thinking_content: Option<&str>,
    tool_calls: Option<&str>,
    is_finished: Option<bool>,
    is_streaming: Option<bool>,
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

    if let Some(content) = content {
        tx.execute("UPDATE messages SET content = ?1 WHERE id = ?2", params![content, message_id])?;
    }
    if let Some(thinking_content) = thinking_content {
        tx.execute("UPDATE messages SET thinking_content = ?1 WHERE id = ?2", params![thinking_content, message_id])?;
    }
    if let Some(tool_calls) = tool_calls {
        tx.execute("UPDATE messages SET tool_calls = ?1 WHERE id = ?2", params![tool_calls, message_id])?;
    }
    if let Some(is_finished) = is_finished {
        tx.execute("UPDATE messages SET is_finished = ?1 WHERE id = ?2", params![is_finished, message_id])?;
    }
    if let Some(is_streaming) = is_streaming {
        tx.execute("UPDATE messages SET is_streaming = ?1 WHERE id = ?2", params![is_streaming, message_id])?;
    }

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

/// Add and/or remove tags on a single tool call within a message.
///
/// Reads the message's `tool_calls` JSON blob, parses it, applies the tag
/// changes to the tool call with the given id, saves the updated blob, and
/// bumps the chat version. Returns the new chat version.
pub(crate) fn update_message_tool_call_tags(
    conn: &Mutex<Connection>,
    message_id: i64,
    tool_call_id: &str,
    add_tags: &[String],
    remove_tags: &[String],
) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;

    let (chat_id, tool_calls_json): (i64, Option<String>) = tx.query_row(
        "SELECT chat_id, tool_calls FROM messages WHERE id = ?1",
        params![message_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|_| DbError::NotFound(format!("message {}", message_id)))?;

    let mut tool_calls: Vec<ToolCall> = match tool_calls_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| DbError::SerializationError(format!("invalid tool_calls JSON for message {}: {}", message_id, e)))?,
        None => return Err(DbError::NotFound(format!(
            "tool call {} in message {} (message has no tool calls)",
            tool_call_id, message_id
        ))),
    };

    let tool_call = tool_calls
        .iter_mut()
        .find(|tc| tc.id == tool_call_id)
        .ok_or_else(|| {
            DbError::NotFound(format!("tool call {} in message {}", tool_call_id, message_id))
        })?;

    for tag in add_tags {
        if !tool_call.tags.contains(tag) {
            tool_call.tags.push(tag.clone());
        }
    }
    tool_call.tags.retain(|tag| !remove_tags.contains(tag));

    let updated_json = serde_json::to_string(&tool_calls)
        .map_err(|e| DbError::SerializationError(format!("failed to serialize tool_calls: {}", e)))?;
    tx.execute(
        "UPDATE messages SET tool_calls = ?1 WHERE id = ?2",
        params![updated_json, message_id],
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

