//! Integration tests for optional chat logging (`proxy.logging`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::Router;
use bytes::Bytes;
use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use rhd_ai_proxy::config::{Logging, ModelConfig, Proxy as ProxyConfig, Target};
use rhd_ai_proxy::logging::{LoggingDb, LoggingError};
use rhd_ai_proxy::proxy::{build_router, ProxyState};

// ─── Upstreams ───

/// Captures the last request body and answers with a fixed chat completion.
#[derive(Clone)]
struct ChatUpstream {
    received: Arc<Mutex<Option<Vec<u8>>>>,
}

async fn chat_json_handler(State(upstream): State<ChatUpstream>, request: Request) -> Response {
    let body = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .unwrap();
    *upstream.received.lock().await = Some(body.to_vec());
    chat_completion_response()
}

fn chat_completion_response() -> Response {
    Json(json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello from model"},
            "finish_reason": "stop"
        }]
    }))
    .into_response()
}

const SSE_BODY: &str = concat!(
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
    "data: [DONE]\n\n",
);

async fn chat_sse_handler() -> Response {
    let stream = async_stream::stream! {
        let (first, second) = SSE_BODY.split_at(SSE_BODY.find("lo").unwrap());
        yield Ok::<_, std::io::Error>(Bytes::from_static(first.as_bytes()));
        yield Ok(Bytes::from_static(second.as_bytes()));
    };
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

// ─── Helpers ───

async fn spawn_upstream(router: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

fn target_base(addr: SocketAddr) -> String {
    format!("http://{addr}/raw/openrouter/v1")
}

fn temp_logging_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rhd_ai_proxy_logging_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn spawn_proxy(
    target_path: String,
    models: HashMap<String, ModelConfig>,
    logging_dir: PathBuf,
) -> (u16, PathBuf) {
    let db_path = logging_dir.join("chats.sqlite3");
    let config = ProxyConfig {
        port: 0,
        target: Target {
            path: target_path,
            api_key: None,
        },
        models,
        logging: Some(Logging { path: logging_dir }),
    };
    let state = Arc::new(ProxyState::new(&config).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.unwrap();
    });
    (port, db_path)
}

fn model_config(extra_body: Value) -> ModelConfig {
    ModelConfig {
        extra_body: serde_json::from_value(extra_body).unwrap(),
    }
}

/// Polls the database until `cond` holds, returning the connection it held on.
async fn wait_for(db_path: &Path, cond: impl Fn(&Connection) -> bool) -> Connection {
    for _ in 0..200 {
        if let Ok(conn) = Connection::open(db_path) {
            if cond(&conn) {
                return conn;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition on {db_path:?} was never met");
}

fn scalar_i64(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

async fn post_json(port: u16, path: &str, body: &[u8]) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .await
        .unwrap()
}

// ─── Tests ───

#[tokio::test]
async fn logs_non_stream_request_with_raw_bodies_and_assembled_message() {
    let dir = temp_logging_dir("non_stream");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let (port, db_path) =
        spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    let body = json!({
        "model": "gpt-4o",
        "messages": [
            {"role": "system", "content": "be brief"},
            {"role": "user", "content": "hello world"}
        ]
    });
    let original = serde_json::to_vec(&body).unwrap();
    let response = post_json(port, "/v1/chat/completions", &original).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    // Consume the body: completion is persisted when the response stream ends.
    assert!(response.text().await.unwrap().contains("Hello from model"));

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests WHERE status IS NOT NULL") == 1
    })
    .await;

    assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM chats"), 1);
    let (title, model): (String, String) = conn
        .query_row("SELECT title, model FROM chats", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(title, "hello world");
    assert_eq!(model, "gpt-4o");

    let (status, stream, duration, assembled): (i64, i64, Option<i64>, String) = conn
        .query_row(
            "SELECT status, stream, duration_ms, response_assembled FROM requests",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(stream, 0);
    assert!(duration.is_some());
    let assembled: Value = serde_json::from_str(&assembled).unwrap();
    assert_eq!(assembled["content"], "Hello from model");

    let (stored_request, stored_response): (Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT request_body, response_body FROM raw",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored_request, original);
    assert_eq!(stored_response, serde_json::to_vec(&json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello from model"},
            "finish_reason": "stop"
        }]
    })).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn groups_chat_continuations_under_same_chat() {
    let dir = temp_logging_dir("grouping");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let (port, db_path) =
        spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    let first = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"}
    ]});
    let continuation = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"},
        {"role": "assistant", "content": "reply"},
        {"role": "user", "content": "second"}
    ]});
    let unrelated = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s2"},
        {"role": "user", "content": "other"}
    ]});

    for body in [&first, &continuation, &unrelated, &first] {
        let response = post_json(
            port,
            "/v1/chat/completions",
            &serde_json::to_vec(body).unwrap(),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let _ = response.text().await.unwrap();
    }

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests") == 4
            && scalar_i64(conn, "SELECT COUNT(*) FROM chats") == 2
    })
    .await;

    // Request order: first, continuation, unrelated, retry-of-first.
    let chat_ids: Vec<i64> = conn
        .prepare("SELECT chat_id FROM requests ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(chat_ids.len(), 4);
    assert_eq!(chat_ids[0], chat_ids[1]);
    assert_eq!(chat_ids[0], chat_ids[3]);
    assert_ne!(chat_ids[0], chat_ids[2]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn captures_streaming_response_after_stream_end() {
    let dir = temp_logging_dir("streaming");
    let addr = spawn_upstream(Router::new().fallback(chat_sse_handler)).await;
    let (port, db_path) =
        spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    let body = json!({
        "model": "gpt-4o",
        "stream": true,
        "messages": [{"role": "user", "content": "hi"}]
    });
    let response = post_json(port, "/v1/chat/completions", &serde_json::to_vec(&body).unwrap())
        .await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let text = response.text().await.unwrap();
    assert_eq!(text, SSE_BODY);

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests WHERE status IS NOT NULL") == 1
    })
    .await;

    let (status, stream, assembled, raw): (i64, i64, String, Vec<u8>) = conn
        .query_row(
            "SELECT status, stream, response_assembled, response_body
             FROM raw JOIN requests ON requests.id = raw.request_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(stream, 1);
    assert_eq!(raw, SSE_BODY.as_bytes());
    let assembled: Value = serde_json::from_str(&assembled).unwrap();
    assert_eq!(assembled["role"], "assistant");
    assert_eq!(assembled["content"], "Hello");
    assert_eq!(assembled["finish_reason"], "stop");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn records_upstream_send_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = listener.local_addr().unwrap();
    drop(listener);
    let dir = temp_logging_dir("send_failure");
    let (port, db_path) =
        spawn_proxy(format!("http://{dead_addr}/v1"), HashMap::new(), dir.clone()).await;

    let body = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]});
    let response = post_json(port, "/v1/chat/completions", &serde_json::to_vec(&body).unwrap())
        .await;
    assert_eq!(response.status(), axum::http::StatusCode::BAD_GATEWAY);

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests WHERE error IS NOT NULL") == 1
    })
    .await;
    let (status, error, raw_response): (Option<i64>, String, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT status, error, response_body FROM raw JOIN requests ON requests.id = raw.request_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, None);
    assert!(error.contains("upstream request failed"));
    assert_eq!(raw_response, Some(Vec::<u8>::new()));
    // The chat and the harness's request were still recorded.
    assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM chats"), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn stores_original_body_before_injection() {
    let dir = temp_logging_dir("pre_injection");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let models = HashMap::from([(
        "gpt-4o".to_string(),
        model_config(json!({"provider": {"sort": "throughput"}})),
    )]);
    let (port, db_path) = spawn_proxy(target_base(addr), models, dir.clone()).await;

    let body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}]
    });
    let original = serde_json::to_vec(&body).unwrap();
    let response = post_json(port, "/v1/chat/completions", &original).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let _ = response.text().await.unwrap();

    // Upstream received the injected body...
    let forwarded = upstream.received.lock().await.take().unwrap();
    let forwarded: Value = serde_json::from_slice(&forwarded).unwrap();
    assert_eq!(forwarded["provider"]["sort"], "throughput");

    // ...while the log stores what the harness actually sent.
    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests WHERE status IS NOT NULL") == 1
    })
    .await;
    let stored: Vec<u8> = conn
        .query_row("SELECT request_body FROM raw", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, original);
    let stored: Value = serde_json::from_slice(&stored).unwrap();
    assert!(stored.get("provider").is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn skips_non_completions_and_unparseable_requests() {
    let dir = temp_logging_dir("skips");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let (port, db_path) = spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/models"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let response = post_json(port, "/v1/chat/completions", b"not json").await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    // Give async logging a chance to (incorrectly) record something.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let conn = Connection::open(&db_path).unwrap();
    assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM requests"), 0);
    assert_eq!(scalar_i64(&conn, "SELECT COUNT(*) FROM chats"), 0);

    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Branch-aware classification ───

#[tokio::test]
async fn subset_history_creates_separate_chat() {
    let dir = temp_logging_dir("subset");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let (port, db_path) = spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    // Parent chat A: seed request, then a continuation that advances its frontier.
    let parent_seed = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"}
    ]});
    let parent_next = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"},
        {"role": "assistant", "content": "reply"},
        {"role": "user", "content": "second"}
    ]});
    // Sub-chat B spawned with a subset of A's messages plus its own task message.
    let sub_seed = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"},
        {"role": "user", "content": "sub task"}
    ]});
    let sub_next = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "first"},
        {"role": "user", "content": "sub task"},
        {"role": "assistant", "content": "sub reply"}
    ]});

    for body in [&parent_seed, &parent_next, &sub_seed, &sub_next] {
        let response = post_json(
            port,
            "/v1/chat/completions",
            &serde_json::to_vec(body).unwrap(),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let _ = response.text().await.unwrap();
    }

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests") == 4
            && scalar_i64(conn, "SELECT COUNT(*) FROM chats") == 2
    })
    .await;

    // Request order: parent seed, parent continuation, sub seed, sub continuation.
    let chat_ids: Vec<i64> = conn
        .prepare("SELECT chat_id FROM requests ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(chat_ids.len(), 4);
    assert_eq!(chat_ids[0], chat_ids[1], "parent requests share a chat");
    assert_eq!(chat_ids[2], chat_ids[3], "sub-chat requests share a chat");
    assert_ne!(
        chat_ids[0], chat_ids[2],
        "the sub-chat must not be merged into the parent"
    );

    // Each chat has its own full message sequence (the sub-chat duplicated the seed
    // messages it was spawned with): four history rows plus one assembled response.
    let count_for = |conn: &Connection, chat_id: i64| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
            [chat_id],
            |row| row.get(0),
        )
        .unwrap()
    };
    let history_count_for = |conn: &Connection, chat_id: i64| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE chat_id = ?1 AND source = 'history'",
            [chat_id],
            |row| row.get(0),
        )
        .unwrap()
    };
    for chat_id in [chat_ids[0], chat_ids[2]] {
        assert_eq!(count_for(&conn, chat_id), 5);
        assert_eq!(history_count_for(&conn, chat_id), 4);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn edited_history_forks_new_chat() {
    let dir = temp_logging_dir("edit_fork");
    let upstream = ChatUpstream {
        received: Arc::new(Mutex::new(None)),
    };
    let addr = spawn_upstream(
        Router::new()
            .fallback(chat_json_handler)
            .with_state(upstream.clone()),
    )
    .await;
    let (port, db_path) = spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    let original = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "original"}
    ]});
    let advanced = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "original"},
        {"role": "user", "content": "more"}
    ]});
    let edited = json!({"model": "gpt-4o", "messages": [
        {"role": "system", "content": "s1"},
        {"role": "user", "content": "edited"}
    ]});

    for body in [&original, &advanced, &edited] {
        let response = post_json(
            port,
            "/v1/chat/completions",
            &serde_json::to_vec(body).unwrap(),
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let _ = response.text().await.unwrap();
    }

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM requests") == 3
            && scalar_i64(conn, "SELECT COUNT(*) FROM chats") == 2
    })
    .await;

    let chat_ids: Vec<i64> = conn
        .prepare("SELECT chat_id FROM requests ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(chat_ids[0], chat_ids[1]);
    assert_ne!(chat_ids[0], chat_ids[2]);

    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Tool calls in the messages table ───

const TOOL_SSE_BODY: &str = concat!(
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\":\\\"Oslo\\\"}\"}}]}}]}\n\n",
    "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    "data: [DONE]\n\n",
);

async fn chat_tool_sse_handler() -> Response {
    let stream = async_stream::stream! {
        yield Ok::<_, std::io::Error>(Bytes::from_static(TOOL_SSE_BODY.as_bytes()));
    };
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

#[tokio::test]
async fn tool_calls_and_results_persisted_to_messages() {
    let dir = temp_logging_dir("tool_calls");
    let addr = spawn_upstream(Router::new().fallback(chat_tool_sse_handler)).await;
    let (port, db_path) = spawn_proxy(target_base(addr), HashMap::new(), dir.clone()).await;

    // First request: the model streams a tool call.
    let first = json!({"model": "gpt-4o", "stream": true, "messages": [
        {"role": "user", "content": "weather in Oslo?"}
    ]});
    let response = post_json(port, "/v1/chat/completions", &serde_json::to_vec(&first).unwrap())
        .await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let _ = response.text().await.unwrap();

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(
            conn,
            "SELECT COUNT(*) FROM messages WHERE source = 'response'",
        ) == 1
    })
    .await;

    // The assembled tool call is queryable in its own column.
    let (role, tool_calls, source): (String, String, String) = conn
        .query_row(
            "SELECT role, tool_calls, source FROM messages WHERE seq = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(role, "assistant");
    assert_eq!(source, "response");
    let tool_calls: Value = serde_json::from_str(&tool_calls).unwrap();
    assert_eq!(tool_calls[0]["id"], "call_1");
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
    assert_eq!(
        tool_calls[0]["function"]["arguments"],
        serde_json::json!({"city": "Oslo"}).to_string()
    );

    // Second request: the harness re-delivers the assistant tool call and the result.
    let second = json!({"model": "gpt-4o", "stream": true, "messages": [
        {"role": "user", "content": "weather in Oslo?"},
        {"role": "assistant", "content": null, "tool_calls": [
            {"id": "call_1", "type": "function",
             "function": {"name": "get_weather", "arguments": "{\"city\":\"Oslo\"}"}}
        ]},
        {"role": "tool", "tool_call_id": "call_1", "content": "{\"temp\": 20}"}
    ]});
    let response = post_json(port, "/v1/chat/completions", &serde_json::to_vec(&second).unwrap())
        .await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let _ = response.text().await.unwrap();

    let conn = wait_for(&db_path, |conn| {
        scalar_i64(conn, "SELECT COUNT(*) FROM messages WHERE role = 'tool'") == 1
    })
    .await;

    let (role, content, tool_call_id, source): (String, String, String, String) = conn
        .query_row(
            "SELECT role, content, tool_call_id, source FROM messages WHERE seq = 2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(role, "tool");
    assert_eq!(content, serde_json::to_string("{\"temp\": 20}").unwrap());
    assert_eq!(tool_call_id, "call_1");
    assert_eq!(source, "history");

    // The re-delivered assistant turn matched the stored response row (finish_reason
    // is stripped for comparison), so it was not rewritten.
    let (source, message_json): (String, String) = conn
        .query_row(
            "SELECT source, message_json FROM messages WHERE seq = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(source, "response");
    assert!(!message_json.contains("finish_reason"));

    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Schema lifecycle ───

#[test]
fn old_schema_database_fails_fast() {
    let dir = temp_logging_dir("old_schema");
    std::fs::create_dir_all(&dir).unwrap();
    let conn = Connection::open(dir.join("chats.sqlite3")).unwrap();
    // Minimal v1-era shape: chats table, no user_version stamp.
    conn.execute_batch(
        "CREATE TABLE chats (id INTEGER PRIMARY KEY, title TEXT NOT NULL, model TEXT,
                             created_at TEXT NOT NULL, updated_at TEXT NOT NULL);",
    )
    .unwrap();
    drop(conn);

    match LoggingDb::open(&dir) {
        Err(LoggingError::IncompatibleSchema { path, found, .. }) => {
            assert!(path.ends_with("chats.sqlite3"));
            assert_eq!(found, 0);
        }
        Err(other) => panic!("expected IncompatibleSchema, got {other}"),
        Ok(_) => panic!("expected IncompatibleSchema, open succeeded"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
