//! SQLite storage for chat logging: schema, connection management, and queries.
//!
//! Follows the project's `Mutex<Connection>` pattern. All methods are synchronous and
//! must be called from `spawn_blocking` in async contexts.
//!
//! Chat attribution is content-based (see [`crate::logging::classify`]); one
//! [`LoggingDb::record_request`] transaction classifies the history, registers prefix
//! hashes, inserts the request row, and appends the history's messages atomically.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::logging::chat_match::{chat_title, prefix_hashes, CHAT_TITLE_MAX_CHARS};
use crate::logging::classify::{classify_and_register, Classification};
use crate::logging::messages::{append_history, append_response};
use crate::logging::schema::apply_schema;

/// File name of the logging database inside the configured `logging.path` folder.
pub const DB_FILE_NAME: &str = "chats.sqlite3";

/// How long a writer waits for the database lock before failing. Long enough for
/// concurrent proxy processes sharing the database to queue their writes.
const BUSY_TIMEOUT: u64 = 5;

/// Errors from opening or writing the logging database.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("failed to create logging directory {path}: {source}")]
    DirCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("logging database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "logging database {path} has schema version {found}, this build writes \
         version {expected}; delete or move the file to start fresh"
    )]
    IncompatibleSchema {
        path: String,
        found: i64,
        expected: i64,
    },
}

/// Handle to the logging database. Cheap to share via `Arc`.
pub struct LoggingDb {
    conn: Mutex<Connection>,
}

/// A request being logged, recorded when the request arrives (before forwarding).
pub struct NewRequest {
    pub ts: String,
    pub method: String,
    pub path: String,
    pub model: Option<String>,
    pub stream: bool,
    pub request_body: Vec<u8>,
}

/// Outcome of recording a request: where it landed and how it was classified.
#[derive(Debug)]
pub struct RecordOutcome {
    pub request_id: i64,
    pub chat_id: i64,
    pub classification: Classification,
}

/// The recorded outcome of a logged request, persisted when its response finishes.
pub struct RequestCompletion {
    pub status: Option<u16>,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub response_body: Vec<u8>,
    pub response_assembled: Option<String>,
}

/// Current UTC timestamp in RFC3339 format.
pub(crate) fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl LoggingDb {
    /// Opens (or creates) the logging database in `dir`.
    ///
    /// Creates the directory when missing, enables WAL and foreign keys, and applies
    /// the schema idempotently. Multiple proxy processes may share one database:
    /// WAL permits concurrent readers plus serialized writers, and the busy timeout
    /// makes concurrent writers queue instead of failing with `SQLITE_BUSY`.
    ///
    /// Fails fast on databases with an unknown schema version (see
    /// [`LoggingError::IncompatibleSchema`]).
    pub fn open(dir: &Path) -> Result<Self, LoggingError> {
        std::fs::create_dir_all(dir).map_err(|source| LoggingError::DirCreate {
            path: dir.display().to_string(),
            source,
        })?;
        let conn = Connection::open(dir.join(DB_FILE_NAME))?;
        conn.busy_timeout(std::time::Duration::from_secs(BUSY_TIMEOUT))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        apply_schema(&conn, &dir.join(DB_FILE_NAME))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Records an incoming request in one transaction: classifies its history
    /// (retry / continuation / branch / new chat), registers new prefix hashes,
    /// inserts the request row with its raw original body, and diff-appends the
    /// history's messages.
    pub fn record_request(
        &self,
        request: &NewRequest,
        messages: &[serde_json::Value],
    ) -> Result<RecordOutcome, LoggingError> {
        let hashes = prefix_hashes(messages);
        let title = chat_title(messages, CHAT_TITLE_MAX_CHARS);
        let mut conn = self.conn.lock().unwrap();
        // IMMEDIATE takes the write lock up front so concurrent proxy processes queue
        // before reading, keeping classification race-free.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let classification =
            classify_and_register(&tx, &hashes, &title, request.model.as_deref())?;
        let chat_id = classification.chat_id();
        tx.execute(
            "INSERT INTO requests (chat_id, ts, method, path, model, stream)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                chat_id,
                request.ts,
                request.method,
                request.path,
                request.model,
                request.stream
            ],
        )?;
        let request_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO raw (request_id, request_body) VALUES (?1, ?2)",
            params![request_id, request.request_body],
        )?;
        append_history(&tx, chat_id, request_id, messages)?;
        tx.commit()?;
        Ok(RecordOutcome {
            request_id,
            chat_id,
            classification,
        })
    }

    /// Persists the outcome of a request: status, timing, raw response bytes, and the
    /// assembled assistant message — which is also appended to the chat's `messages`
    /// sequence at `response_seq` (the originating request's history length). Also
    /// touches the chat's `updated_at` and `model`.
    pub fn complete_request(
        &self,
        request_id: i64,
        response_seq: i64,
        completion: &RequestCompletion,
    ) -> Result<(), LoggingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE requests SET status = ?1, duration_ms = ?2, error = ?3, response_assembled = ?4
             WHERE id = ?5",
            params![
                completion.status.map(i64::from),
                completion.duration_ms,
                completion.error,
                completion.response_assembled,
                request_id
            ],
        )?;
        tx.execute(
            "UPDATE raw SET response_body = ?2 WHERE request_id = ?1",
            params![request_id, completion.response_body],
        )?;
        tx.execute(
            "UPDATE chats
             SET updated_at = ?2,
                 model = COALESCE((SELECT model FROM requests WHERE id = ?1), model)
             WHERE id = (SELECT chat_id FROM requests WHERE id = ?1)",
            params![request_id, now_rfc3339()],
        )?;
        let chat_id: Option<i64> = tx
            .query_row(
                "SELECT chat_id FROM requests WHERE id = ?1",
                params![request_id],
                |row| row.get(0),
            )
            .optional()?;
        if let (Some(chat_id), Some(assembled)) = (
            chat_id,
            completion
                .response_assembled
                .as_deref()
                .and_then(|assembled| serde_json::from_str::<serde_json::Value>(assembled).ok()),
        ) {
            append_response(&tx, chat_id, request_id, response_seq, &assembled)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Direct connection access for unit tests within this crate.
    #[cfg(test)]
    pub(crate) fn conn_for_tests(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

#[cfg(test)]
 mod tests {
    use super::*;
    use crate::logging::schema::SCHEMA_VERSION;
    use serde_json::json;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_db_test_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn new_request(model: Option<&str>, body: &[u8]) -> NewRequest {
        NewRequest {
            ts: now_rfc3339(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: model.map(str::to_string),
            stream: false,
            request_body: body.to_vec(),
        }
    }

    fn messages(turns: &[&str]) -> Vec<serde_json::Value> {
        let mut messages = vec![json!({"role": "system", "content": "sys"})];
        for turn in turns {
            messages.push(json!({"role": "user", "content": turn}));
        }
        messages
    }

    #[test]
    fn open_creates_versioned_database_and_is_idempotent() {
        let dir = temp_dir("open");
        LoggingDb::open(&dir).unwrap();
        let version: i64 = Connection::open(dir.join(DB_FILE_NAME))
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        // Re-open applies schema idempotently.
        LoggingDb::open(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn record_request_lifecycle_inserts_then_completes() {
        let dir = temp_dir("lifecycle");
        let db = LoggingDb::open(&dir).unwrap();

        let history = messages(&["hello"]);
        let outcome = db
            .record_request(&new_request(Some("gpt-4o"), b"original-bytes"), &history)
            .unwrap();
        assert!(matches!(
            outcome.classification,
            Classification::NewChat { .. }
        ));

        {
            let conn = db.conn_for_tests();
            let (status, error): (Option<i64>, Option<String>) = conn
                .query_row(
                    "SELECT status, error FROM requests WHERE id = ?1",
                    params![outcome.request_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(status, None);
            assert_eq!(error, None);
            let body: Vec<u8> = conn
                .query_row(
                    "SELECT request_body FROM raw WHERE request_id = ?1",
                    params![outcome.request_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(body, b"original-bytes");
            let history_rows: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
                    params![outcome.chat_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(history_rows, 2);
        }

        db.complete_request(
            outcome.request_id,
            2,
            &RequestCompletion {
                status: Some(200),
                duration_ms: 42,
                error: None,
                response_body: b"response-bytes".to_vec(),
                response_assembled: Some(
                    r#"{"role":"assistant","content":"hi","finish_reason":"stop"}"#.to_string(),
                ),
            },
        )
        .unwrap();

        {
            let conn = db.conn_for_tests();
            let (status, duration, assembled, response): (i64, i64, String, Vec<u8>) = conn
                .query_row(
                    "SELECT status, duration_ms, response_assembled,
                            (SELECT response_body FROM raw WHERE request_id = requests.id)
                     FROM requests WHERE id = ?1",
                    params![outcome.request_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(status, 200);
            assert_eq!(duration, 42);
            assert!(assembled.contains("assistant"));
            assert_eq!(response, b"response-bytes");
            // The assembled response became a response-sourced message row.
            let (role, source, message_json): (String, String, String) = conn
                .query_row(
                    "SELECT role, source, message_json FROM messages
                     WHERE chat_id = ?1 AND seq = 2",
                    params![outcome.chat_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(role, "assistant");
            assert_eq!(source, "response");
            assert!(!message_json.contains("finish_reason"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multiple_handles_share_one_database() {
        // Two handles on the same file model two proxy processes: writes from one
        // are visible to the other, and interleaved writes queue instead of failing.
        let dir = temp_dir("shared");
        let first = LoggingDb::open(&dir).unwrap();
        let second = LoggingDb::open(&dir).unwrap();

        let history = messages(&["shared chat"]);
        let outcome = first
            .record_request(&new_request(Some("gpt-4o"), b"body"), &history)
            .unwrap();

        // The other handle continues the same chat.
        let mut extended = history.clone();
        extended.push(json!({"role": "user", "content": "more"}));
        let continuation = second
            .record_request(&new_request(Some("gpt-4o"), b"body2"), &extended)
            .unwrap();
        assert_eq!(
            continuation.classification,
            Classification::Continuation {
                chat_id: outcome.chat_id
            }
        );

        // And the first handle completes it.
        first
            .complete_request(
                continuation.request_id,
                3,
                &RequestCompletion {
                    status: Some(200),
                    duration_ms: 1,
                    error: None,
                    response_body: b"ok".to_vec(),
                    response_assembled: None,
                },
            )
            .unwrap();

        let conn = Connection::open(dir.join(DB_FILE_NAME)).unwrap();
        let status: i64 = conn
            .query_row(
                "SELECT status FROM requests WHERE id = ?1",
                params![continuation.request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, 200);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
