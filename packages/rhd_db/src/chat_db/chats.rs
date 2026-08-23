use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use super::ChatInfo;
use crate::{DbError, DbResult};

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn create_chat(conn: &Mutex<Connection>, title: &str) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO chats (title, created_at, updated_at, version) VALUES (?1, ?2, ?3, 1)",
        params![title, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn list_chats(conn: &Mutex<Connection>) -> DbResult<Vec<ChatInfo>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, active_model, version FROM chats ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ChatInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            active_model: row.get(4)?,
            version: row.get(5)?,
        })
    })?;
    let mut chats = Vec::new();
    for row in rows {
        chats.push(row?);
    }
    Ok(chats)
}

pub(crate) fn get_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<Option<ChatInfo>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, active_model, version FROM chats WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(ChatInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            active_model: row.get(4)?,
            version: row.get(5)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub(crate) fn delete_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
    Ok(())
}

pub(crate) fn update_chat_title(conn: &Mutex<Connection>, id: i64, title: &str) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET title = ?1, updated_at = ?2, version = version + 1 WHERE id = ?3",
        params![title, now, id],
    )?;
    // Return the new version
    let mut stmt = conn.prepare("SELECT version FROM chats WHERE id = ?1")?;
    let version: i64 = stmt.query_row(params![id], |row| row.get(0))?;
    Ok(version)
}

pub(crate) fn touch_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        params![now, id],
    )?;
    // Return the new version
    let mut stmt = conn.prepare("SELECT version FROM chats WHERE id = ?1")?;
    let version: i64 = stmt.query_row(params![id], |row| row.get(0))?;
    Ok(version)
}

/// Increment the version of a chat and return the new version.
pub(crate) fn increment_chat_version(conn: &Mutex<Connection>, id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "UPDATE chats SET version = version + 1, updated_at = ?1 WHERE id = ?2",
        params![now_iso(), id],
    )?;
    // Return the new version
    let mut stmt = conn.prepare("SELECT version FROM chats WHERE id = ?1")?;
    let version: i64 = stmt.query_row(params![id], |row| row.get(0))?;
    Ok(version)
}

/// Result of conditional get_chat_if_version_higher query.
pub enum ChatVersionResult {
    /// Chat state is newer than requested version.
    NewerVersion(ChatInfo),
    /// Chat state matches requested version (actual).
    Actual,
}

/// Get chat only if its version is higher than the specified version.
/// Returns ChatVersionResult::NewerVersion if current version > if_version_higher_than.
/// Returns ChatVersionResult::Actual if current version == if_version_higher_than.
/// Returns error if current version < if_version_higher_than (should not happen).
pub(crate) fn get_chat_if_version_higher(
    conn: &Mutex<Connection>,
    id: i64,
    if_version_higher_than: i64,
) -> DbResult<ChatVersionResult> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, active_model, version FROM chats WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(ChatInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            active_model: row.get(4)?,
            version: row.get(5)?,
        })
    })?;
    match rows.next() {
        Some(row) => {
            let chat = row?;
            if chat.version > if_version_higher_than {
                Ok(ChatVersionResult::NewerVersion(chat))
            } else if chat.version == if_version_higher_than {
                Ok(ChatVersionResult::Actual)
            } else {
                Err(DbError::InitializationError(format!(
                    "Chat version {} is lower than requested {}",
                    chat.version, if_version_higher_than
                )))
            }
        }
        None => Err(DbError::InitializationError(format!(
            "Chat with id {} not found",
            id
        ))),
    }
}

