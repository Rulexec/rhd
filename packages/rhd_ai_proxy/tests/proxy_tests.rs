//! End-to-end tests: proxy in front of local upstream servers.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::Router;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

use rhd_ai_proxy::config::{ModelConfig, Proxy as ProxyConfig, Target};
use rhd_ai_proxy::proxy::{build_router, ProxyState};

// ─── Echo upstream ───

struct ReceivedRequest {
    host: String,
    path: String,
    query: Option<String>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ReceivedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn body_json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

#[derive(Clone)]
struct EchoUpstream {
    received: Arc<Mutex<Option<ReceivedRequest>>>,
}

impl EchoUpstream {
    fn new() -> Self {
        Self {
            received: Arc::new(Mutex::new(None)),
        }
    }

    async fn received_request(&self) -> ReceivedRequest {
        let mut guard = self.received.lock().await;
        guard.take().expect("upstream should have received a request")
    }
}

async fn echo_handler(State(upstream): State<EchoUpstream>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let raw = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let received = ReceivedRequest {
        host: parts
            .headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        path: parts.uri.path().to_string(),
        query: parts.uri.query().map(str::to_string),
        headers: parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect(),
        body: raw.to_vec(),
    };
    *upstream.received.lock().await = Some(received);
    Json(json!({"echo": "ok"})).into_response()
}

// ─── SSE upstream ───

struct SseUpstream {
    release: Arc<Mutex<oneshot::Receiver<()>>>,
}

async fn sse_handler(State(sse): State<Arc<SseUpstream>>) -> Response {
    let stream = async_stream::stream! {
        yield Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"data: MARK_ONE\n\n"));
        let mut guard = sse.release.lock().await;
        let _ = (&mut *guard).await;
        yield Ok(axum::body::Bytes::from_static(b"data: MARK_TWO\n\ndata: [DONE]\n\n"));
    };
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

// ─── Error upstream ───

async fn error_handler() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "upstream exploded").into_response()
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

async fn spawn_proxy(target_path: String, models: HashMap<String, ModelConfig>) -> u16 {
    let config = ProxyConfig {
        port: 0,
        target: Target { path: target_path },
        models,
    };
    let state = Arc::new(ProxyState::new(&config).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, build_router(state)).await.unwrap();
    });
    port
}

fn model_config(extra_body: Value) -> ModelConfig {
    ModelConfig {
        extra_body: serde_json::from_value(extra_body).unwrap(),
    }
}

fn provider_extra_body() -> Value {
    json!({"provider": {"sort": "throughput", "max_price": {"prompt": 1, "completion": 2}}})
}

// ─── Tests ───

#[tokio::test]
async fn injects_extra_body_and_rewrites_host_and_path() {
    let upstream = EchoUpstream::new();
    let addr = spawn_upstream(Router::new().fallback(echo_handler).with_state(upstream.clone())).await;
    let models = HashMap::from([("gpt-4o".to_string(), model_config(provider_extra_body()))]);
    let proxy_port = spawn_proxy(target_base(addr), models).await;

    let response = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{proxy_port}/v1/chat/completions"
        ))
        .header("authorization", "Bearer test-key")
        .json(&json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let received = upstream.received_request().await;
    assert_eq!(received.path, "/raw/openrouter/v1/chat/completions");
    assert_eq!(received.host, addr.to_string());
    assert_eq!(received.header("authorization"), Some("Bearer test-key"));

    let body = received.body_json();
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["messages"][0]["content"], "Hello");
    assert_eq!(body["provider"]["sort"], "throughput");
    assert_eq!(body["provider"]["max_price"]["prompt"], 1);
    assert_eq!(body["provider"]["max_price"]["completion"], 2);
}

#[tokio::test]
async fn passes_through_non_matching_model_unchanged() {
    let upstream = EchoUpstream::new();
    let addr = spawn_upstream(Router::new().fallback(echo_handler).with_state(upstream.clone())).await;
    let models = HashMap::from([
        (
            "z-ai/glm-5.3".to_string(),
            model_config(provider_extra_body()),
        ),
    ]);
    let proxy_port = spawn_proxy(target_base(addr), models).await;

    let original = json!({"model": "gpt-4o", "messages": []});
    let response = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{proxy_port}/v1/chat/completions"
        ))
        .json(&original)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let received = upstream.received_request().await;
    assert_eq!(received.body_json(), original);
}

#[tokio::test]
async fn preserves_query_params_and_skips_non_completions_paths() {
    let upstream = EchoUpstream::new();
    let addr = spawn_upstream(Router::new().fallback(echo_handler).with_state(upstream.clone())).await;
    let models = HashMap::from([("gpt-4o".to_string(), model_config(provider_extra_body()))]);
    let proxy_port = spawn_proxy(target_base(addr), models).await;

    let original = json!({"model": "gpt-4o"});
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{proxy_port}/v1/models?foo=bar"))
        .json(&original)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let received = upstream.received_request().await;
    assert_eq!(received.path, "/raw/openrouter/v1/models");
    assert_eq!(received.query.as_deref(), Some("foo=bar"));
    assert_eq!(received.body_json(), original);
}

#[tokio::test]
async fn streams_sse_response_incrementally() {
    let (release_tx, release_rx) = oneshot::channel();
    let sse = Arc::new(SseUpstream {
        release: Arc::new(Mutex::new(release_rx)),
    });
    let addr = spawn_upstream(Router::new().fallback(sse_handler).with_state(sse)).await;
    let proxy_port = spawn_proxy(target_base(addr), HashMap::new()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{proxy_port}/v1/chat/completions"
        ))
        .json(&json!({"model": "gpt-4o", "messages": [], "stream": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap().to_str().unwrap(),
        "text/event-stream"
    );

    let mut stream = response.bytes_stream();
    let first = stream.next().await.unwrap().unwrap();
    let first_text = String::from_utf8_lossy(&first).to_string();
    assert!(first_text.contains("MARK_ONE"));
    assert!(!first_text.contains("MARK_TWO"));

    release_tx.send(()).unwrap();

    let mut rest = Vec::new();
    while let Some(chunk) = stream.next().await {
        rest.extend(chunk.unwrap().to_vec());
    }
    let rest_text = String::from_utf8_lossy(&rest);
    assert!(rest_text.contains("MARK_TWO"));
    assert!(rest_text.contains("[DONE]"));
}

#[tokio::test]
async fn forwards_upstream_error_status_and_body() {
    let addr = spawn_upstream(Router::new().fallback(error_handler)).await;
    let proxy_port = spawn_proxy(target_base(addr), HashMap::new()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{proxy_port}/v1/chat/completions"
        ))
        .json(&json!({"model": "gpt-4o", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.text().await.unwrap(), "upstream exploded");
}

#[tokio::test]
async fn returns_bad_gateway_when_upstream_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = listener.local_addr().unwrap();
    drop(listener);
    let proxy_port = spawn_proxy(format!("http://{dead_addr}/v1"), HashMap::new()).await;

    let response = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{proxy_port}/v1/chat/completions"
        ))
        .json(&json!({"model": "gpt-4o", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}
