//! Logging database schema: DDL, version stamping, and the open-time version guard.
//!
//! Schema version 2 introduced branch-aware chat identity (`prefix_hashes.len`) and
//! the normalized `messages` table. The version is stamped in `PRAGMA user_version`;
//! databases at any other version are rejected at open — delete or move the file to
//! start fresh (no in-place migration).

use std::path::Path;

use rusqlite::Connection;

/// Schema version written to `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS chats (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    model      TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Prefix-hash chain per chat: element i covers messages 0..=i. Single owner per hash,
-- first registrant wins; a chat's frontier is its MAX(len) row.
CREATE TABLE IF NOT EXISTS prefix_hashes (
    hash    BLOB PRIMARY KEY,
    chat_id INTEGER NOT NULL REFERENCES chats(id),
    len     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_prefix_hashes_chat ON prefix_hashes(chat_id, len);

CREATE TABLE IF NOT EXISTS requests (
    id                 INTEGER PRIMARY KEY,
    chat_id            INTEGER NOT NULL REFERENCES chats(id),
    ts                 TEXT NOT NULL,
    method             TEXT NOT NULL,
    path               TEXT NOT NULL,
    model              TEXT,
    stream             INTEGER NOT NULL DEFAULT 0,
    status             INTEGER,
    duration_ms        INTEGER,
    error              TEXT,
    response_assembled TEXT
);

CREATE TABLE IF NOT EXISTS raw (
    request_id    INTEGER PRIMARY KEY REFERENCES requests(id),
    request_body  BLOB NOT NULL,
    response_body BLOB
);

-- Normalized conversation per chat: request histories diff-appended, plus assembled
-- responses. message_json is the comparison basis; the rest are projections.
CREATE TABLE IF NOT EXISTS messages (
    id           INTEGER PRIMARY KEY,
    chat_id      INTEGER NOT NULL REFERENCES chats(id),
    seq          INTEGER NOT NULL,
    role         TEXT NOT NULL,
    message_json TEXT NOT NULL,
    content      TEXT,
    tool_calls   TEXT,
    tool_call_id TEXT,
    name         TEXT,
    source       TEXT NOT NULL CHECK (source IN ('history', 'response')),
    request_id   INTEGER NOT NULL REFERENCES requests(id),
    UNIQUE (chat_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_requests_chat ON requests(chat_id, id);
";

/// Applies the schema on a fresh database and enforces the version guard. The
/// connection must already have WAL and foreign keys configured; `db_path` is the
/// database file path (used in error messages).
pub fn apply_schema(
    conn: &Connection,
    db_path: &Path,
) -> Result<(), crate::logging::db::LoggingError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    let has_chats: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chats')",
        [],
        |row| row.get(0),
    )?;
    match (version, has_chats) {
        // Fresh file: apply the schema and stamp the version.
        (0, false) => {
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            Ok(())
        }
        // Version 0 with tables is a v1 database from before versioning; any other
        // version is equally incompatible.
        _ => Err(crate::logging::db::LoggingError::IncompatibleSchema {
            path: db_path.display().to_string(),
            found: version,
            expected: SCHEMA_VERSION,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::db::LoggingError;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_schema_test_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open_raw(dir: &Path) -> Connection {
        Connection::open(dir.join("chats.sqlite3")).unwrap()
    }

    fn version_of(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn fresh_database_gets_schema_and_version_stamp() {
        let dir = temp_dir("fresh");
        let conn = open_raw(&dir);
        apply_schema(&conn, &dir.join("chats.sqlite3")).unwrap();
        assert_eq!(version_of(&conn), SCHEMA_VERSION);
        // Idempotent on re-apply.
        apply_schema(&conn, &dir.join("chats.sqlite3")).unwrap();
        assert_eq!(version_of(&conn), SCHEMA_VERSION);

        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN
                 ('chats', 'prefix_hashes', 'requests', 'raw', 'messages')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_database_without_version_is_rejected() {
        let dir = temp_dir("v1");
        let conn = open_raw(&dir);
        conn.execute_batch(
            "CREATE TABLE chats (id INTEGER PRIMARY KEY, title TEXT NOT NULL, model TEXT,
                                 created_at TEXT NOT NULL, updated_at TEXT NOT NULL);",
        )
        .unwrap();
        drop(conn);

        let conn = open_raw(&dir);
        match apply_schema(&conn, &dir.join("chats.sqlite3")) {
            Err(LoggingError::IncompatibleSchema { path, found, .. }) => {
                assert!(path.ends_with("chats.sqlite3"));
                assert_eq!(found, 0);
            }
            other => panic!("expected IncompatibleSchema, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn future_schema_version_is_rejected() {
        let dir = temp_dir("future");
        let conn = open_raw(&dir);
        apply_schema(&conn, &dir.join("chats.sqlite3")).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        drop(conn);

        let conn = open_raw(&dir);
        assert!(matches!(
            apply_schema(&conn, &dir.join("chats.sqlite3")),
            Err(LoggingError::IncompatibleSchema { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
