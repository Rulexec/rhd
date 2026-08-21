use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// Custom event information from the database.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventInfo {
    pub event_id: String,
    pub event_name: String,
    pub sender_plugin_id: Option<String>,
    pub additional: Option<String>,
    pub created_at: String,
}

/// Create a new custom event.
pub fn create_custom_event(
    conn: &Mutex<Connection>,
    event_id: &str,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "INSERT INTO custom_events (event_id, event_name, sender_plugin_id, additional) VALUES (?, ?, ?, ?)",
        rusqlite::params![event_id, event_name, sender_plugin_id, additional],
    )?;
    Ok(())
}

/// Get a custom event by ID.
pub fn get_custom_event(conn: &Mutex<Connection>, event_id: &str) -> DbResult<Option<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT event_id, event_name, sender_plugin_id, additional, created_at FROM custom_events WHERE event_id = ?"
    )?;
    let event = stmt
        .query_map([event_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                created_at: row.get::<_, String>(4)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(event)
}

/// Acknowledge a custom event for a plugin.
pub fn ack_custom_event(conn: &Mutex<Connection>, event_id: &str, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "INSERT OR IGNORE INTO custom_event_acks (event_id, plugin_id) VALUES (?, ?)",
        [event_id, plugin_id],
    )?;
    Ok(())
}

/// Check if a plugin has acknowledged a custom event.
pub fn has_plugin_acked(conn: &Mutex<Connection>, event_id: &str, plugin_id: &str) -> DbResult<bool> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let count: i64 = conn_guard
        .prepare("SELECT COUNT(*) FROM custom_event_acks WHERE event_id = ? AND plugin_id = ?")?
        .query_row(rusqlite::params![event_id, plugin_id], |row| row.get(0))?;
    Ok(count > 0)
}

/// Get all custom events that a plugin has NOT acknowledged.
pub fn get_pending_events_for_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<Vec<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT ce.event_id, ce.event_name, ce.sender_plugin_id, ce.additional, ce.created_at
         FROM custom_events ce
         WHERE ce.event_id NOT IN (
             SELECT event_id FROM custom_event_acks WHERE plugin_id = ?
         )
         ORDER BY ce.created_at"
    )?;
    let events = stmt
        .query_map([plugin_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                created_at: row.get::<_, String>(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

/// Delete a custom event (also deletes all acknowledgments due to CASCADE).
pub fn delete_custom_event(conn: &Mutex<Connection>, event_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute("DELETE FROM custom_events WHERE event_id = ?", [event_id])?;
    Ok(())
}
