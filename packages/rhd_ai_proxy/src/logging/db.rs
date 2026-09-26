//! SQLite storage for chat logging: schema, connection management, and queries.
//!
//! Follows the project's `Mutex<Connection>` pattern. All methods are synchronous and
//! must be called from `spawn_blocking` in async contexts.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

/// File name of the logging database inside the configured `logging.path` folder.
pub const DB_FILE_NAME: &str = "chats.sqlite3";

/// How long a writer waits for the database lock before failing. Long enough for
/// concurrent proxy processes sharing the database to queue their writes.
const BUSY_TIMEOUT: u64 = 5;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS chats (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    model      TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS prefix_hashes (
    hash    BLOB PRIMARY KEY,
    chat_id INTEGER NOT NULL REFERENCES chats(id)
);

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

CREATE INDEX IF NOT EXISTS idx_requests_chat ON requests(chat_id, id);
";

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
}

/// Handle to the logging database. Cheap to share via `Arc`.
pub struct LoggingDb {
    conn: Mutex<Connection>,
}

/// A request being logged, inserted when the request arrives (before forwarding).
pub struct NewRequest {
    pub chat_id: i64,
    pub ts: String,
    pub method: String,
    pub path: String,
    pub model: Option<String>,
    pub stream: bool,
    pub request_body: Vec<u8>,
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
    pub fn open(dir: &Path) -> Result<Self, LoggingError> {
        std::fs::create_dir_all(dir).map_err(|source| LoggingError::DirCreate {
            path: dir.display().to_string(),
            source,
        })?;
        let conn = Connection::open(dir.join(DB_FILE_NAME))?;
        conn.busy_timeout(std::time::Duration::from_secs(BUSY_TIMEOUT))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Finds the chat for the longest known prefix hash, checking longest-first.
    ///
    /// Returns `None` when no prefix of the request's history was seen before
    /// (the request starts a new chat).
    pub fn find_chat_id(&self, prefix_hashes: &[[u8; 32]]) -> Result<Option<i64>, LoggingError> {
        let conn = self.conn.lock().unwrap();
        for hash in prefix_hashes.iter().rev() {
            let mut stmt =
                conn.prepare_cached("SELECT chat_id FROM prefix_hashes WHERE hash = ?1")?;
            let chat_id = stmt
                .query_row(params![hash.as_slice()], |row| row.get::<_, i64>(0))
                .optional()?;
            if let Some(chat_id) = chat_id {
                return Ok(Some(chat_id));
            }
        }
        Ok(None)
    }

    /// Creates a new chat and returns its id.
    pub fn create_chat(&self, title: &str, model: Option<&str>) -> Result<i64, LoggingError> {
        let conn = self.conn.lock().unwrap();
        let now = now_rfc3339();
        conn.execute(
            "INSERT INTO chats (title, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![title, model, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Registers prefix hashes → `chat_id`. Hashes shared with an earlier chat are
    /// re-pointed to `chat_id` (`INSERT OR REPLACE`), so the most recently registered
    /// chat wins on shared prefixes.
    pub fn register_prefixes(&self, chat_id: i64, hashes: &[[u8; 32]]) -> Result<(), LoggingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO prefix_hashes (hash, chat_id) VALUES (?1, ?2)",
            )?;
            for hash in hashes {
                stmt.execute(params![hash.as_slice(), chat_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Inserts a request row together with its raw original request body.
    pub fn insert_request(&self, request: &NewRequest) -> Result<i64, LoggingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO requests (chat_id, ts, method, path, model, stream)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                request.chat_id,
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
        tx.commit()?;
        Ok(request_id)
    }

    /// Persists the outcome of a request: status, timing, raw response bytes, and the
    /// assembled assistant message. Also touches the chat's `updated_at` and `model`.
    pub fn complete_request(
        &self,
        request_id: i64,
        completion: &RequestCompletion,
    ) -> Result<(), LoggingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
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
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn hashes_for(messages: &[serde_json::Value]) -> Vec<[u8; 32]> {
        crate::logging::chat_match::prefix_hashes(messages)
    }

    fn messages(short: &str) -> Vec<serde_json::Value> {
        vec![json!({"role": "system", "content": "sys"}), json!({"role": "user", "content": short})]
    }

    #[test]
    fn open_creates_database_file_and_is_idempotent() {
        let dir = temp_dir("open");
        LoggingDb::open(&dir).unwrap();
        assert!(dir.join(DB_FILE_NAME).exists());
        // Re-open applies schema idempotently.
        LoggingDb::open(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_chat_id_matches_longest_prefix() {
        let dir = temp_dir("longest");
        let db = LoggingDb::open(&dir).unwrap();

        let first = hashes_for(&messages("first"));
        let chat_a = db.create_chat("a", Some("m1")).unwrap();
        db.register_prefixes(chat_a, &first).unwrap();

        let mut extended = messages("first");
        extended.push(json!({"role": "user", "content": "second turn"}));
        let extended_hashes = hashes_for(&extended);

        // Longest match: the extended history still matches on the shared prefix.
        assert_eq!(db.find_chat_id(&extended_hashes).unwrap(), Some(chat_a));

        // But a different history only shares the system message prefix.
        let other = hashes_for(&messages("other"));
        let chat_b = db.create_chat("b", None).unwrap();
        db.register_prefixes(chat_b, &other).unwrap();
        assert_eq!(db.find_chat_id(&other).unwrap(), Some(chat_b));

        // Unrelated history (new system message) matches nothing.
        let mut unrelated = messages("whatever");
        unrelated[0] = json!({"role": "system", "content": "different"});
        assert_eq!(db.find_chat_id(&hashes_for(&unrelated)).unwrap(), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_prefix_repoints_to_latest_chat() {
        let dir = temp_dir("repoint");
        let db = LoggingDb::open(&dir).unwrap();

        let hashes = hashes_for(&messages("same start"));
        let chat_a = db.create_chat("a", None).unwrap();
        db.register_prefixes(chat_a, &hashes).unwrap();
        let chat_b = db.create_chat("b", None).unwrap();
        db.register_prefixes(chat_b, &hashes).unwrap();

        assert_eq!(db.find_chat_id(&hashes).unwrap(), Some(chat_b));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multiple_handles_share_one_database() {
        // Two handles on the same file model two proxy processes: writes from one
        // are visible to the other, and interleaved writes queue instead of failing.
        let dir = temp_dir("shared");
        let first = LoggingDb::open(&dir).unwrap();
        let second = LoggingDb::open(&dir).unwrap();

        let hashes = hashes_for(&messages("shared chat"));
        let chat_id = first.create_chat("shared chat", Some("gpt-4o")).unwrap();
        first.register_prefixes(chat_id, &hashes).unwrap();

        // The other handle continues the same chat and records a request.
        assert_eq!(second.find_chat_id(&hashes).unwrap(), Some(chat_id));
        let request_id = second
            .insert_request(&NewRequest {
                chat_id,
                ts: now_rfc3339(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                model: Some("gpt-4o".to_string()),
                stream: false,
                request_body: b"body".to_vec(),
            })
            .unwrap();

        // And the first handle completes it.
        first.complete_request(
            request_id,
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
                params![request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, 200);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn request_lifecycle_insert_then_complete() {
        let dir = temp_dir("lifecycle");
        let db = LoggingDb::open(&dir).unwrap();

        let hashes = hashes_for(&messages("hello"));
        let chat_id = db.create_chat("hello", Some("gpt-4o")).unwrap();
        db.register_prefixes(chat_id, &hashes).unwrap();

        let request_id = db
            .insert_request(&NewRequest {
                chat_id,
                ts: now_rfc3339(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                model: Some("gpt-4o".to_string()),
                stream: false,
                request_body: b"original-bytes".to_vec(),
            })
            .unwrap();

        {
            let conn = db.conn.lock().unwrap();
            let (status, error): (Option<i64>, Option<String>) = conn
                .query_row(
                    "SELECT status, error FROM requests WHERE id = ?1",
                    params![request_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(status, None);
            assert_eq!(error, None);
            let body: Vec<u8> = conn
                .query_row(
                    "SELECT request_body FROM raw WHERE request_id = ?1",
                    params![request_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(body, b"original-bytes");
        }

        db.complete_request(
            request_id,
            &RequestCompletion {
                status: Some(200),
                duration_ms: 42,
                error: None,
                response_body: b"response-bytes".to_vec(),
                response_assembled: Some(r#"{"role":"assistant"}"#.to_string()),
            },
        )
        .unwrap();

        {
            let conn = db.conn.lock().unwrap();
            let (status, duration, assembled, response): (i64, i64, String, Vec<u8>) = conn
                .query_row(
                    "SELECT status, duration_ms, response_assembled,
                            (SELECT response_body FROM raw WHERE request_id = requests.id)
                     FROM requests WHERE id = ?1",
                    params![request_id],
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
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
