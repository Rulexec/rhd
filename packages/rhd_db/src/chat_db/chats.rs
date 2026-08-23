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
        "INSERT INTO chats (title, created_at, updated_at) VALUES (?1, ?2, ?3)",
        params![title, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn list_chats(conn: &Mutex<Connection>) -> DbResult<Vec<ChatInfo>> {
    let conn = conn
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

pub(crate) fn get_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<Option<ChatInfo>> {
    let conn = conn
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

pub(crate) fn delete_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
    Ok(())
}

pub(crate) fn update_chat_title(conn: &Mutex<Connection>, id: i64, title: &str) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    Ok(())
}

pub(crate) fn touch_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

