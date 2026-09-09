use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// A stored plugin state row (tombstones included when fetched by single key).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateRow {
    pub plugin_id: String,
    pub key: String,
    pub content: String,
    /// "markdown" | "json" — validated by the CHECK constraint.
    pub format: String,
    pub schema: String,
    pub version: i64,
    pub is_removed: bool,
    /// SQLite `datetime('now')` string, e.g. "2026-09-05 22:41:07".
    pub updated_at: String,
}

const ROW_COLUMNS: &str = "plugin_id, key, content, format, schema, version, is_removed, updated_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PluginStateRow> {
    Ok(PluginStateRow {
        plugin_id: row.get::<_, String>(0)?,
        key: row.get::<_, String>(1)?,
        content: row.get::<_, String>(2)?,
        format: row.get::<_, String>(3)?,
        schema: row.get::<_, String>(4)?,
        version: row.get::<_, i64>(5)?,
        is_removed: row.get::<_, i64>(6)? != 0,
        updated_at: row.get::<_, String>(7)?,
    })
}

/// Upsert a state. Fresh insert gets version 1; every update bumps the version
/// and clears any tombstone (monotonic across delete/re-create).
pub fn upsert_plugin_state(
    conn: &Mutex<Connection>,
    plugin_id: &str,
    key: &str,
    content: &str,
    format: &str,
    schema: &str,
) -> DbResult<PluginStateRow> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;

    conn_guard.execute(
        "INSERT INTO plugin_states (plugin_id, key, content, format, schema, version, is_removed, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, datetime('now'))
         ON CONFLICT(plugin_id, key) DO UPDATE SET
             content = excluded.content,
             format = excluded.format,
             schema = excluded.schema,
             version = plugin_states.version + 1,
             is_removed = 0,
             updated_at = datetime('now')",
        rusqlite::params![plugin_id, key, content, format, schema],
    )?;

    let mut stmt = conn_guard.prepare(&format!(
        "SELECT {ROW_COLUMNS} FROM plugin_states WHERE plugin_id = ?1 AND key = ?2"
    ))?;
    let row = stmt.query_row(rusqlite::params![plugin_id, key], map_row)?;
    Ok(row)
}

/// Tombstone a live state, bumping its version.
/// Returns the new version, or None if there was nothing live to remove.
pub fn remove_plugin_state(
    conn: &Mutex<Connection>,
    plugin_id: &str,
    key: &str,
) -> DbResult<Option<i64>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;

    let affected = conn_guard.execute(
        "UPDATE plugin_states SET is_removed = 1, version = version + 1, updated_at = datetime('now')
         WHERE plugin_id = ?1 AND key = ?2 AND is_removed = 0",
        rusqlite::params![plugin_id, key],
    )?;

    if affected == 0 {
        return Ok(None);
    }
    let version: i64 = conn_guard.query_row(
        "SELECT version FROM plugin_states WHERE plugin_id = ?1 AND key = ?2",
        rusqlite::params![plugin_id, key],
        |row| row.get(0),
    )?;
    Ok(Some(version))
}

/// All live (non-tombstone) states, optionally filtered by plugin and/or schema.
pub fn get_plugin_states(
    conn: &Mutex<Connection>,
    plugin_id: Option<&str>,
    schema: Option<&str>,
) -> DbResult<Vec<PluginStateRow>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;

    let sql = format!(
        "SELECT {ROW_COLUMNS} FROM plugin_states
         WHERE is_removed = 0
           AND (?1 IS NULL OR plugin_id = ?1)
           AND (?2 IS NULL OR schema = ?2)
         ORDER BY plugin_id, key"
    );
    let mut stmt = conn_guard.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![plugin_id, schema], map_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Single row by exact key, INCLUDING tombstones — used by the subscribe
/// catch-up comparison (a tombstone's version still gates stale events).
pub fn get_plugin_state(
    conn: &Mutex<Connection>,
    plugin_id: &str,
    key: &str,
) -> DbResult<Option<PluginStateRow>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(&format!(
        "SELECT {ROW_COLUMNS} FROM plugin_states WHERE plugin_id = ?1 AND key = ?2"
    ))?;
    let mut rows = stmt.query_map(rusqlite::params![plugin_id, key], map_row)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}
