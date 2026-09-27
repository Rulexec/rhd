//! Normalized message rows for logged chats.
//!
//! The `messages` table mirrors each chat's conversation: request histories are
//! diff-appended (first-seen order), and completed responses append the assembled
//! assistant message. The request history is authoritative — when a resent history
//! disagrees with stored rows, everything from the divergence point is replaced.
//!
//! Each row stores the full canonical message (`message_json`, with top-level
//! `finish_reason` stripped so SSE-assembled and history-delivered forms of the same
//! turn compare equal) plus extracted projection columns: `role`, `content`,
//! `tool_calls` (assistant turns), and `tool_call_id`/`name` (tool results).

use rusqlite::{params, Connection};
use serde_json::Value;

/// Source of a message row: delivered inside a request history, or assembled from a
/// response.
const SOURCE_HISTORY: &str = "history";
const SOURCE_RESPONSE: &str = "response";

/// Canonical serialization used for storage and comparison: sorted keys (workspace
/// `serde_json`) with top-level `finish_reason` removed.
pub fn comparable_canonical(message: &Value) -> String {
    let mut clone = message.clone();
    if let Value::Object(map) = &mut clone {
        map.remove("finish_reason");
    }
    serde_json::to_string(&clone).unwrap_or_default()
}

/// Projection columns extracted from one message value.
struct MessageFields {
    role: String,
    /// The `content` field as verbatim JSON text (string or array of parts); `None`
    /// when absent or null.
    content: Option<String>,
    /// The `tool_calls` array as JSON text; `None` when absent, null, or empty.
    tool_calls: Option<String>,
    tool_call_id: Option<String>,
    name: Option<String>,
}

fn extract_fields(message: &Value) -> MessageFields {
    MessageFields {
        role: message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        content: message
            .get("content")
            .filter(|content| !content.is_null())
            .map(Value::to_string),
        tool_calls: message
            .get("tool_calls")
            .and_then(Value::as_array)
            .filter(|calls| !calls.is_empty())
            .map(|calls| Value::Array(calls.clone()).to_string()),
        tool_call_id: message
            .get("tool_call_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        name: message
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// Diff-appends a request history to the chat's message sequence.
///
/// Stored rows that match the incoming messages positionally (by canonical form) are
/// kept; the first mismatch triggers truncate-and-replace from that position (the
/// request history is authoritative), and messages beyond the stored count are
/// appended. A pure resend changes nothing.
pub fn append_history(
    conn: &Connection,
    chat_id: i64,
    request_id: i64,
    messages: &[Value],
) -> Result<(), rusqlite::Error> {
    let stored: Vec<String> = {
        let mut stmt = conn.prepare_cached(
            "SELECT message_json FROM messages WHERE chat_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };

    // First position where the history is new (beyond stored rows) or disagrees with
    // storage; everything from there on is (re-)written.
    let start = match messages
        .iter()
        .enumerate()
        .find(|(index, message)| stored.get(*index) != Some(&comparable_canonical(message)))
    {
        Some((start, _)) => start,
        None => return Ok(()),
    };

    if start < stored.len() {
        tracing::warn!(
            chat_id,
            seq = start,
            "request history diverges from stored messages; replacing from this position"
        );
        let mut stmt =
            conn.prepare_cached("DELETE FROM messages WHERE chat_id = ?1 AND seq >= ?2")?;
        stmt.execute(params![chat_id, start as i64])?;
    }
    for (index, message) in messages.iter().enumerate().skip(start) {
        insert_message(conn, chat_id, request_id, index as i64, message, SOURCE_HISTORY)?;
    }
    Ok(())
}

/// Appends the assembled assistant message from a completed response at sequence
/// position `seq` (the length of the originating request's history, so a re-completed
/// response targets its own slot). First writer wins: a slot already occupied (e.g.
/// the message arrived earlier via another request's history) is left untouched.
pub fn append_response(
    conn: &Connection,
    chat_id: i64,
    request_id: i64,
    seq: i64,
    message: &Value,
) -> Result<(), rusqlite::Error> {
    insert_message(conn, chat_id, request_id, seq, message, SOURCE_RESPONSE)?;
    Ok(())
}

/// Inserts one message row, ignoring a `(chat_id, seq)` conflict.
fn insert_message(
    conn: &Connection,
    chat_id: i64,
    request_id: i64,
    seq: i64,
    message: &Value,
    source: &str,
) -> Result<(), rusqlite::Error> {
    let canonical = comparable_canonical(message);
    let fields = extract_fields(message);
    let mut stmt = conn.prepare_cached(
        "INSERT INTO messages
             (chat_id, seq, role, message_json, content, tool_calls, tool_call_id, name,
              source, request_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (chat_id, seq) DO NOTHING",
    )?;
    stmt.execute(params![
        chat_id,
        seq,
        fields.role,
        canonical,
        fields.content,
        fields.tool_calls,
        fields.tool_call_id,
        fields.name,
        source,
        request_id
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Opens a database seeded with chat 1 and requests 10-12 (message FK targets),
    /// cleaning up the temp directory on drop. Each test gets its own directory.
    struct TestDb {
        logging: crate::logging::db::LoggingDb,
        dir: PathBuf,
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn db(name: &str) -> TestDb {
        let dir = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_messages_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let logging = crate::logging::db::LoggingDb::open(&dir).unwrap();
        {
            let conn = logging.conn_for_tests();
            let now = "2026-01-01T00:00:00Z";
            conn.execute(
                "INSERT INTO chats (id, title, model, created_at, updated_at)
                 VALUES (1, 'test', NULL, ?1, ?1)",
                params![now],
            )
            .unwrap();
            for request_id in [10i64, 11, 12] {
                conn.execute(
                    "INSERT INTO requests (id, chat_id, ts, method, path, stream)
                     VALUES (?1, 1, ?2, 'POST', '/v1/chat/completions', 0)",
                    params![request_id, now],
                )
                .unwrap();
            }
        }
        TestDb { logging, dir }
    }

    /// Runs `f` inside one committed transaction; the connection guard is released
    /// before returning so follow-up read helpers can lock it.
    fn run(test: &TestDb, f: impl FnOnce(&Connection)) {
        let conn = test.logging.conn_for_tests();
        let tx = conn.unchecked_transaction().unwrap();
        f(&tx);
        tx.commit().unwrap();
    }

    /// `(seq, role, source)` rows of the chat in sequence order.
    fn rows(test: &TestDb, chat_id: i64) -> Vec<(i64, String, String)> {
        let conn = test.logging.conn_for_tests();
        let mut stmt = conn
            .prepare("SELECT seq, role, source FROM messages WHERE chat_id = ?1 ORDER BY seq")
            .unwrap();
        stmt.query_map(params![chat_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    fn history_2() -> Vec<Value> {
        vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "hi"}),
        ]
    }

    #[test]
    fn appends_only_new_trailing_messages() {
        let test = db("append");
        run(&test, |conn| {
            append_history(conn, 1, 10, &history_2()).unwrap();
        });
        assert_eq!(
            rows(&test, 1),
            vec![
                (0, "system".to_string(), "history".to_string()),
                (1, "user".to_string(), "history".to_string()),
            ]
        );

        // A continuation re-delivers the prefix and adds one message.
        let mut extended = history_2();
        extended.push(json!({"role": "assistant", "content": "hello"}));
        run(&test, |conn| {
            append_history(conn, 1, 11, &extended).unwrap();
        });
        let stored = rows(&test, 1);
        assert_eq!(stored.len(), 3);
        assert_eq!(
            stored[2],
            (2, "assistant".to_string(), "history".to_string())
        );

        // A pure resend changes nothing.
        run(&test, |conn| {
            append_history(conn, 1, 12, &extended).unwrap();
        });
        assert_eq!(rows(&test, 1).len(), 3);
    }

    #[test]
    fn mismatched_history_truncates_and_replaces() {
        let test = db("truncate");
        let mut history = history_2();
        history.push(json!({"role": "assistant", "content": "original reply"}));
        run(&test, |conn| {
            append_history(conn, 1, 10, &history).unwrap();
        });
        assert_eq!(rows(&test, 1).len(), 3);

        // An edited resend replaces the assistant turn (and anything after it).
        let mut edited = history_2();
        edited.push(json!({"role": "assistant", "content": "edited reply"}));
        edited.push(json!({"role": "user", "content": "next"}));
        run(&test, |conn| {
            append_history(conn, 1, 11, &edited).unwrap();
        });
        let stored = rows(&test, 1);
        assert_eq!(stored.len(), 4);
        assert_eq!(stored[2].1, "assistant");
        assert_eq!(stored[3].1, "user");
        let conn = test.logging.conn_for_tests();
        let message_json: String = conn
            .query_row(
                "SELECT message_json FROM messages WHERE chat_id = 1 AND seq = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(message_json.contains("edited reply"));
    }

    #[test]
    fn response_row_dedups_against_history_redelivery() {
        let test = db("dedup");
        let response = json!({
            "role": "assistant", "content": "reply", "finish_reason": "stop"
        });
        run(&test, |conn| {
            append_history(conn, 1, 10, &history_2()).unwrap();
            append_response(conn, 1, 10, 2, &response).unwrap();
        });
        let stored = rows(&test, 1);
        assert_eq!(stored.len(), 3);
        assert_eq!(
            stored[2],
            (2, "assistant".to_string(), "response".to_string())
        );

        // The next request's history re-delivers the assistant turn — same canonical
        // form once finish_reason is stripped — so nothing is rewritten.
        let mut next = history_2();
        next.push(json!({"role": "assistant", "content": "reply"}));
        next.push(json!({"role": "user", "content": "next"}));
        run(&test, |conn| {
            append_history(conn, 1, 11, &next).unwrap();
        });
        let stored = rows(&test, 1);
        assert_eq!(stored.len(), 4);
        // The response-sourced row was kept as-is (matched, not replaced).
        assert_eq!(stored[2].2, "response");

        // A second completion of the same response targets its own slot and does not
        // duplicate.
        run(&test, |conn| {
            append_response(conn, 1, 10, 2, &response).unwrap();
        });
        assert_eq!(rows(&test, 1).len(), 4);
    }

    #[test]
    fn extracts_tool_calls_and_tool_results() {
        let test = db("tools");
        let history = vec![
            json!({"role": "user", "content": "weather?"}),
            json!({"role": "assistant", "content": null, "tool_calls": [
                {"id": "call_1", "type": "function",
                 "function": {"name": "get_weather", "arguments": "{\"city\":\"Oslo\"}"}}
            ]}),
            json!({"role": "tool", "tool_call_id": "call_1", "name": "get_weather",
                   "content": "{\"temp\": 20}"}),
        ];
        run(&test, |conn| {
            append_history(conn, 1, 10, &history).unwrap();
        });

        let conn = test.logging.conn_for_tests();
        let (role, content, tool_calls, tool_call_id, name): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT role, content, tool_calls, tool_call_id, name
                 FROM messages WHERE chat_id = 1 AND seq = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(role, "assistant");
        // Null content is stored as SQL NULL.
        assert_eq!(content, None);
        let tool_calls: Value = serde_json::from_str(&tool_calls.unwrap()).unwrap();
        assert_eq!(tool_calls[0]["id"], "call_1");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
        assert_eq!(tool_call_id, None);
        assert_eq!(name, None);

        let (role, content, tool_call_id, name): (String, String, String, String) = conn
            .query_row(
                "SELECT role, content, tool_call_id, name
                 FROM messages WHERE chat_id = 1 AND seq = 2",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(role, "tool");
        // String content is stored as its verbatim JSON form (quoted, escaped).
        assert_eq!(content, serde_json::to_string("{\"temp\": 20}").unwrap());
        assert_eq!(tool_call_id, "call_1");
        assert_eq!(name, "get_weather");
    }
}
