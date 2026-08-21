use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// Plugin information from the database.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub plugin_id: String,
    pub is_active: bool,
    pub created_at: String,
}

/// Register a plugin or mark it as active if it already exists.
pub fn register_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    // Check if plugin exists
    let exists: bool = conn_guard
        .prepare("SELECT COUNT(*) FROM plugins WHERE plugin_id = ?")?
        .query_row([plugin_id], |row| row.get::<_, i64>(0))?
        > 0;
    
    if exists {
        // Mark as active
        conn_guard.execute(
            "UPDATE plugins SET is_active = 1 WHERE plugin_id = ?",
            [plugin_id],
        )?;
    } else {
        // Insert new plugin
        conn_guard.execute(
            "INSERT INTO plugins (plugin_id, is_active) VALUES (?, 1)",
            [plugin_id],
        )?;
    }
    
    Ok(())
}

/// Mark a plugin as inactive (called when WebSocket disconnects).
pub fn deactivate_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "UPDATE plugins SET is_active = 0 WHERE plugin_id = ?",
        [plugin_id],
    )?;
    Ok(())
}

/// Remove a plugin completely.
pub fn remove_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute("DELETE FROM plugins WHERE plugin_id = ?", [plugin_id])?;
    Ok(())
}

/// Get all plugins.
pub fn get_plugins(conn: &Mutex<Connection>) -> DbResult<Vec<PluginInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT plugin_id, is_active, created_at FROM plugins ORDER BY created_at"
    )?;
    let plugins = stmt
        .query_map([], |row| {
            Ok(PluginInfo {
                plugin_id: row.get::<_, String>(0)?,
                is_active: row.get::<_, i64>(1)? != 0,
                created_at: row.get::<_, String>(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(plugins)
}

/// Get a specific plugin.
pub fn get_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<Option<PluginInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT plugin_id, is_active, created_at FROM plugins WHERE plugin_id = ?"
    )?;
    let plugin = stmt
        .query_map([plugin_id], |row| {
            Ok(PluginInfo {
                plugin_id: row.get::<_, String>(0)?,
                is_active: row.get::<_, i64>(1)? != 0,
                created_at: row.get::<_, String>(2)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(plugin)
}

/// Check if a plugin exists and is active.
pub fn is_plugin_active(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<bool> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let is_active: Option<bool> = conn_guard
        .prepare("SELECT is_active FROM plugins WHERE plugin_id = ?")?
        .query_row([plugin_id], |row| Ok(row.get::<_, i64>(0)? != 0))
        .ok();
    Ok(is_active.unwrap_or(false))
}
