# Phase 2: rhd_db — plugin_states Table & CRUD

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Persist plugin states in SQLite: new `plugin_states` table with server-managed
monotonic `version` and tombstone-based removal, plus typed access functions and
`ChatDb` wrapper methods. No protocol/serde concerns here — this layer stores raw
rows; Phase 3 maps rows to `rhd_chat_api::PluginState`.

**Scope in:** schema, `plugin_states.rs`, `ChatDb` methods, DB tests.
**Scope out:** handlers, events, API types (Phase 1 defines `PluginState`).

**Dependencies:** none (independent of Phase 1 — uses plain Rust types).
Blocks: Phase 3.

## Versioning Rules (implemented here)

1. Fresh insert → `version = 1`.
2. Update of existing row (live or tombstone) → `version = old + 1`, `is_removed = 0`.
3. Remove of a live row → `is_removed = 1`, `version = old + 1` (tombstone kept).
4. Remove of an already-removed or nonexistent row → no-op, returns `None`.
5. Re-create after remove continues the sequence (v3 → removed v4 → re-created v5),
   preserving strict monotonicity per `(plugin_id, key)`.
6. `remove_plugin` (existing) cascades via FK — rows physically deleted, and a
   later re-registration of the same plugin id starts versions from 1 again.
7. `deactivate_plugin` (disconnect) does **not** touch states — last-known state
   survives for the UI.

## Files to Modify/Create

### 1. `packages/rhd_db/src/chat_db/schema.rs` (modify)

Insert after the `chat_tools` block, before `migrate(conn)?`:

```rust
    // Create plugin_states table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS plugin_states (
            plugin_id TEXT NOT NULL,
            key TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            format TEXT NOT NULL DEFAULT 'json' CHECK (format IN ('markdown','json')),
            schema TEXT NOT NULL DEFAULT '',
            version INTEGER NOT NULL DEFAULT 1,
            is_removed INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (plugin_id, key),
            FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_plugin_states_schema ON plugin_states(schema);",
    )?;
```

No `migrate()` additions needed — `CREATE TABLE IF NOT EXISTS` covers fresh and
existing databases.

### 2. `packages/rhd_db/src/chat_db/plugin_states.rs` (new)

```rust
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

fn row_from(rusqlite::Row<'_>) -> rusqlite::Result<PluginStateRow> {
    Ok(PluginStateRow {
        plugin_id: rusqlite::Row::get::<_, String>(0)?, // placeholder — see map below
        // ... fields in the order of ROW_COLUMNS
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

    let mut sql = format!("SELECT {ROW_COLUMNS} FROM plugin_states WHERE is_removed = 0");
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::new();
    if let Some(p) = plugin_id {
        sql.push_str(" AND plugin_id = ?");
        params_vec.push(p);
    }
    if let Some(s) = schema {
        sql.push_str(" AND schema = ?");
        params_vec.push(s);
    }
    sql.push_str(" ORDER BY plugin_id, key");

    let mut stmt = conn_guard.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), map_row)?;
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
```

Replace the `row_from` placeholder with a real `map_row(row: &rusqlite::Row) ->
rusqlite::Result<PluginStateRow>` reading the 8 columns in `ROW_COLUMNS` order
(`is_removed` as `i64` → `!= 0`), exactly like `plugins::get_plugins` does for
`is_active`.

### 3. `packages/rhd_db/src/chat_db/mod.rs` (modify)

- Module list: add `mod plugin_states;` (alphabetical, after `plugins`).
- Re-export: `pub use plugin_states::PluginStateRow;` next to `pub use plugins::PluginInfo;`.

### 3b. `packages/rhd_db/src/lib.rs` (modify — REQUIRED)

`chat_db` is a **private** module (`mod chat_db;` at `lib.rs:1`); types cross the
crate boundary only via the root re-export line. Without this, Phase 3 cannot
name `PluginStateRow`:

```rust
pub use chat_db::{ChatDb, ChatInfo, FunctionCall, Message, PluginStateRow, ToolCall, ToolDefinition, FunctionDefinition};
```
- After the "Plugin operations" block add:

```rust
    // Plugin state operations
    pub fn upsert_plugin_state(
        &self,
        plugin_id: &str,
        key: &str,
        content: &str,
        format: &str,
        schema: &str,
    ) -> DbResult<PluginStateRow> {
        plugin_states::upsert_plugin_state(&self.conn, plugin_id, key, content, format, schema)
    }

    pub fn remove_plugin_state(&self, plugin_id: &str, key: &str) -> DbResult<Option<i64>> {
        plugin_states::remove_plugin_state(&self.conn, plugin_id, key)
    }

    pub fn get_plugin_states(
        &self,
        plugin_id: Option<&str>,
        schema: Option<&str>,
    ) -> DbResult<Vec<PluginStateRow>> {
        plugin_states::get_plugin_states(&self.conn, plugin_id, schema)
    }

    pub fn get_plugin_state(&self, plugin_id: &str, key: &str) -> DbResult<Option<PluginStateRow>> {
        plugin_states::get_plugin_state(&self.conn, plugin_id, key)
    }
```

### 4. `packages/rhd_db/src/chat_db/tests/plugin_state_tests.rs` (new)

Use `crate::ChatDb::new(":memory:")` (pattern already used by the server tests).
Every test first `db.register_plugin("p1")` — the FK requires it.

```rust
use crate::ChatDb;

#[test]
fn first_upsert_has_version_one() { /* ... */ }

#[test]
fn update_bumps_version_and_replaces_fields() {
    // upsert(format "json", schema "a") -> v1; upsert(format "markdown", schema "b") -> v2, content/format/schema replaced
}

#[test]
fn remove_tombstones_bumps_version_and_hides_from_get() {
    // v1 -> remove -> Some(2); get_plugin_states empty; get_plugin_state returns row with is_removed == true
}

#[test]
fn remove_is_noop_on_missing_or_already_removed() {
    // remove unknown key -> None; remove twice -> second is None
}

#[test]
fn recreate_after_remove_is_monotonic() {
    // v1, remove -> v2 tombstone, upsert again -> v3, is_removed == false
}

#[test]
fn keys_are_namespaced_per_plugin() {
    // p1/status and p2/status are independent rows with independent versions
}

#[test]
fn get_filters_by_plugin_and_schema() {
    // two plugins, two schemas; verify each filter combination
}

#[test]
fn remove_plugin_cascades_states() {
    // upsert, then db.remove_plugin("p1"); get_plugin_states -> empty;
    // re-register p1; upsert same key -> version 1 again (sequence reset by cascade)
}

#[test]
fn deactivate_plugin_keeps_states() {
    // upsert, deactivate_plugin, get_plugin_states still returns v1
}
```

### 5. `packages/rhd_db/src/chat_db/tests/mod.rs` (modify)

Add `mod plugin_state_tests;` alongside the existing test modules.

## Tests

```bash
cargo test -p rhd_db plugin_state
mise run check-cargo
```

## Implementation Notes

1. **Tombstones are the whole point of the table design** — without them, a
   re-created state would restart at v1 and version-gated consumers would discard
   it as stale. Test `recreate_after_remove_is_monotonic` locks this in.
2. **`remove_plugin_state` returns `Option<i64>`** (new version) rather than
   `bool` because the handler broadcasts `pluginStateRemoved { version }`.
3. **Dynamic SQL in `get_plugin_states`** uses positional `?` in filter order —
   keep `params_vec` pushes in the same order as the clauses.
4. **`ON CONFLICT` upsert is a single statement** — no read-modify-write race
   even though `ChatDb` serializes on a `Mutex` anyway.
5. **FK cascade** relies on `PRAGMA foreign_keys=ON` (already set in `schema::init`).
6. The CHECK constraint makes an invalid `format` a DB error — Phase 3 maps the
   API enum to the two lowercase strings and trusts the DB on read back.

## Dependencies

- Depends on: nothing (Phase 1 not required for compilation; the API mapping
  lives in Phase 3).
- Blocks: Phase 3 (handlers call these four `ChatDb` methods).
