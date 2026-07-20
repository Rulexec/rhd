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

pub(crate) fn get_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<Option<ChatInfo>> {
    let conn = conn
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

pub(crate) fn delete_chat(conn: &Mutex<Connection>, id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
    Ok(())
}

pub(crate) fn delete_all_chats(conn: &Mutex<Connection>) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute("DELETE FROM chats", [])?;
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

pub(crate) fn update_chat_active_model(conn: &Mutex<Connection>, id: i64, model: &str) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET active_model = ?1, updated_at = ?2 WHERE id = ?3",
        params![model, now, id],
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

pub(crate) fn set_active_role(conn: &Mutex<Connection>, chat_id: i64, project_name: &str, role_name: &str) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET active_role_project = ?1, active_role_name = ?2, updated_at = ?3 WHERE id = ?4",
        params![project_name, role_name, now, chat_id],
    )?;
    Ok(())
}

pub(crate) fn clear_active_role(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET active_role_project = NULL, active_role_name = NULL, updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    Ok(())
}

pub(crate) fn get_active_role(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Option<(String, String)>> {
    let conn = conn
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

pub(crate) fn mark_roles_list_injected(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "UPDATE chats SET roles_list_injected = 1 WHERE id = ?1",
        params![chat_id],
    )?;
    Ok(())
}

pub(crate) fn has_roles_list_been_injected(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<bool> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let injected: bool = conn
        .prepare("SELECT roles_list_injected FROM chats WHERE id = ?1")?
        .query_row(params![chat_id], |row| row.get(0))?;
    Ok(injected)
}

pub(crate) fn reset_roles_list_injected(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "UPDATE chats SET roles_list_injected = 0 WHERE id = ?1",
        params![chat_id],
    )?;
    Ok(())
}

pub(crate) fn set_role_prompt_pending(conn: &Mutex<Connection>, chat_id: i64, pending: bool) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "UPDATE chats SET role_prompt_pending = ?1 WHERE id = ?2",
        params![pending, chat_id],
    )?;
    Ok(())
}

pub(crate) fn has_role_prompt_pending(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<bool> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let pending: bool = conn
        .prepare("SELECT role_prompt_pending FROM chats WHERE id = ?1")?
        .query_row(params![chat_id], |row| row.get(0))?;
    Ok(pending)
}

pub(crate) fn set_todo_list(conn: &Mutex<Connection>, chat_id: i64, todo_list: &str) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET todo_list = ?1, updated_at = ?2 WHERE id = ?3",
        params![todo_list, now, chat_id],
    )?;
    Ok(())
}

pub(crate) fn get_todo_list(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Option<String>> {
    let conn = conn
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
