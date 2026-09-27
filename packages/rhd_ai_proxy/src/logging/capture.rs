//! Request lifecycle logging: record a log record when a completions request arrives,
//! tee the response stream to capture raw bytes, and persist everything when the
//! exchange finishes.
//!
//! All database work runs on the blocking pool; failures are logged via `tracing` and
//! never affect the proxied request.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};

use crate::logging::chat_match::extract_candidate;
use crate::logging::db::{now_rfc3339, LoggingDb, NewRequest, RequestCompletion};
use crate::logging::sse::{assemble_sse_message, extract_message_json};

/// In-flight logging state for one proxied request.
pub struct LoggingContext {
    db: Arc<LoggingDb>,
    request_id: i64,
    chat_id: i64,
    /// Message count of the request's history; the assembled response belongs at this
    /// sequence position in the chat's `messages`.
    response_seq: i64,
    started: Instant,
}

/// Starts logging a completions request: extracts the chat candidate, records the
/// request (classifying its history into a retry / continuation / branch / new chat),
/// and stores the original (pre-injection) raw body.
///
/// Returns `None` when the body is not a loggable chat-completions request or when
/// logging fails (already reported via `tracing`).
pub async fn start_logging(
    db: Arc<LoggingDb>,
    method: &str,
    path: &str,
    raw_body: &[u8],
    started: Instant,
) -> Option<LoggingContext> {
    let candidate = extract_candidate(raw_body)?;
    let request = NewRequest {
        ts: now_rfc3339(),
        method: method.to_string(),
        path: path.to_string(),
        model: candidate.model.clone(),
        stream: candidate.stream,
        request_body: raw_body.to_vec(),
    };
    let response_seq = candidate.messages.len() as i64;
    let messages = candidate.messages;

    let result = tokio::task::spawn_blocking({
        let db = Arc::clone(&db);
        move || db.record_request(&request, &messages)
    })
    .await;

    match result {
        Ok(Ok(outcome)) => {
            tracing::debug!(
                chat_id = outcome.chat_id,
                request_id = outcome.request_id,
                classification = ?outcome.classification,
                "logged incoming request"
            );
            Some(LoggingContext {
                db,
                request_id: outcome.request_id,
                chat_id: outcome.chat_id,
                response_seq,
                started,
            })
        }
        Ok(Err(err)) => {
            tracing::warn!(%err, "failed to log incoming request");
            None
        }
        Err(err) => {
            tracing::warn!(%err, "logging task panicked");
            None
        }
    }
}

/// Drives `upstream` on a spawned task that passes every chunk through to the returned
/// client body unchanged while accumulating the raw bytes, then persists the exchange
/// once the upstream stream ends.
///
/// The capture task drains the upstream independently of client consumption: when the
/// client stops reading early (e.g. a satisfied `content-length` or a disconnect), the
/// log record still completes — on disconnect it records what was received so far.
pub fn capture_body<S>(
    upstream: S,
    ctx: LoggingContext,
    status: u16,
    response_is_json: bool,
) -> Body
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut buffer: Vec<u8> = Vec::new();
        let mut upstream = std::pin::pin!(upstream);
        let mut stream_error: Option<String> = None;
        while let Some(item) = upstream.next().await {
            match &item {
                Ok(chunk) => buffer.extend_from_slice(chunk),
                Err(err) => stream_error = Some(err.to_string()),
            }
            if tx.send(item).is_err() {
                // Client went away; stop draining and record what was received.
                break;
            }
        }
        finish_logging(ctx, Some(status), response_is_json, buffer, stream_error).await;
    });
    let body = async_stream::stream! {
        while let Some(item) = rx.recv().await {
            yield item;
        }
    };
    Body::from_stream(body)
}

/// Records a proxy-level failure (upstream connect error) for a started log record.
pub async fn fail_logging(ctx: LoggingContext, error: String) {
    finish_logging(ctx, None, false, Vec::new(), Some(error)).await;
}

/// Persists the completed exchange on the blocking pool.
async fn finish_logging(
    ctx: LoggingContext,
    status: Option<u16>,
    response_is_json: bool,
    response_body: Vec<u8>,
    error: Option<String>,
) {
    let assembled = if response_is_json {
        extract_message_json(&response_body)
    } else {
        assemble_sse_message(&response_body)
    }
    .map(|value| value.to_string());
    let duration_ms = ctx.started.elapsed().as_millis() as i64;
    let db = ctx.db;
    let request_id = ctx.request_id;
    let chat_id = ctx.chat_id;
    let response_seq = ctx.response_seq;
    let result = tokio::task::spawn_blocking(move || {
        db.complete_request(
            request_id,
            response_seq,
            &RequestCompletion {
                status,
                duration_ms,
                error,
                response_body,
                response_assembled: assembled,
            },
        )
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            tracing::warn!(%err, request_id, chat_id, "failed to log response");
        }
        Err(err) => {
            tracing::warn!(%err, request_id, chat_id, "response logging task panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_capture_test_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn start_logging_creates_chat_and_request_row() {
        let dir = temp_dir("start");
        let db = Arc::new(LoggingDb::open(&dir).unwrap());
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "be brief"},
                {"role": "user", "content": "hello"}
            ]
        });
        let raw = serde_json::to_vec(&body).unwrap();

        let ctx = start_logging(db.clone(), "POST", "/v1/chat/completions", &raw, Instant::now())
            .await
            .expect("logging should start");

        let conn = rusqlite::Connection::open(dir.join("chats.sqlite3")).unwrap();
        let (count, title): (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), MAX(title) FROM chats",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(title, "hello");
        let (requests, stored_body): (i64, Vec<u8>) = conn
            .query_row(
                "SELECT COUNT(*), (SELECT request_body FROM raw LIMIT 1) FROM requests",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(requests, 1);
        assert_eq!(stored_body, raw);
        let history_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
                rusqlite::params![ctx.chat_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(history_rows, 2);
        assert!(ctx.request_id > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn start_logging_returns_none_for_non_chat_body() {
        let dir = temp_dir("skip");
        let db = Arc::new(LoggingDb::open(&dir).unwrap());
        assert!(
            start_logging(db, "POST", "/v1/models", b"{\"object\": \"list\"}", Instant::now())
                .await
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
