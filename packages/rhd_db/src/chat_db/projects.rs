use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::{DbError, DbResult};

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn attach_project(
    conn: &Mutex<Connection>,
    chat_id: i64,
    project_name: &str,
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "INSERT OR IGNORE INTO chat_projects (chat_id, project_name, system_prompt_added, attached_at) VALUES (?1, ?2, 0, ?3)",
        params![chat_id, project_name, now],
    )?;
    Ok(())
}

pub(crate) fn detach_project(
    conn: &Mutex<Connection>,
    chat_id: i64,
    project_name: &str,
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "DELETE FROM chat_projects WHERE chat_id = ?1 AND project_name = ?2",
        params![chat_id, project_name],
    )?;
    Ok(())
}

pub(crate) fn get_chat_projects(
    conn: &Mutex<Connection>,
    chat_id: i64,
) -> DbResult<Vec<(String, bool)>> {
    let conn = conn
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

pub(crate) fn mark_system_prompt_added(
    conn: &Mutex<Connection>,
    chat_id: i64,
    project_name: &str,
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn.execute(
        "UPDATE chat_projects SET system_prompt_added = 1 WHERE chat_id = ?1 AND project_name = ?2",
        params![chat_id, project_name],
    )?;
    Ok(())
}
