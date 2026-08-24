use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::post,
    Json, Router,
};
use futures_util::stream::Stream;
use rhd_ai_client::{ChatCompletionRequest, StreamChunk};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::listener::MockAiListener;
use crate::types::MockAiResponse;

/// Mock AI provider server
pub struct MockAiProvider {
    port: u16,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl MockAiProvider {
    /// Start the mock server with the given listener
    pub async fn start<L: MockAiListener>(listener: L) -> Result<Self, MockAiError> {
        let listener: Arc<dyn MockAiListener> = Arc::new(listener);

        let app = Router::new()
            .route("/v1/chat/completions", post(handle_chat_completion))
            .with_state(listener);

        let tcp_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| MockAiError::Bind(e.to_string()))?;
        let port = tcp_listener
            .local_addr()
            .map_err(|e| MockAiError::Bind(e.to_string()))?
            .port();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            axum::serve(tcp_listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        Ok(Self {
            port,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Get the port the server is running on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the base URL for the API (includes /v1 prefix)
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    /// Shutdown the server
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for MockAiProvider {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MockAiError {
    #[error("failed to bind server: {0}")]
    Bind(String),
}

/// Handler for chat completion requests
async fn handle_chat_completion(
    State(listener): State<Arc<dyn MockAiListener>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    let response = listener.on_chat_completion(request).await;

    match response {
        MockAiResponse::Completion(completion) => Json(completion).into_response(),
        MockAiResponse::Stream(receiver) => {
            let stream = create_sse_stream(receiver);
            Sse::new(stream)
                .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
                .into_response()
        }
        MockAiResponse::Error { status, message } => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, message).into_response()
        }
    }
}

/// Create an SSE stream from a chunk receiver
fn create_sse_stream(
    mut receiver: mpsc::Receiver<StreamChunk>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        while let Some(chunk) = receiver.recv().await {
            let event = chunk_to_sse_event(&chunk);
            yield Ok(event);

            // If this chunk has a finish_reason, send [DONE] and stop
            if chunk.finish_reason.is_some() {
                yield Ok(Event::default().data("[DONE]"));
                break;
            }
        }

        // If receiver was closed without finish_reason, still send [DONE]
        yield Ok(Event::default().data("[DONE]"));
    }
}

/// Convert a StreamChunk to an SSE event
fn chunk_to_sse_event(chunk: &StreamChunk) -> Event {
    let mut delta = serde_json::Map::new();

    if let Some(ref reasoning_content) = chunk.reasoning_content {
        delta.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning_content.clone()));
    }

    if let Some(ref content) = chunk.content {
        delta.insert("content".to_string(), serde_json::Value::String(content.clone()));
    }

    if let Some(ref tool_calls) = chunk.tool_calls {
        let tool_calls_json: Vec<serde_json::Value> = tool_calls
            .iter()
            .map(|tc| {
                let mut tc_map = serde_json::Map::new();
                tc_map.insert("index".to_string(), serde_json::Value::Number(tc.index.into()));

                if let Some(ref id) = tc.id {
                    tc_map.insert("id".to_string(), serde_json::Value::String(id.clone()));
                }

                if let Some(ref call_type) = tc.call_type {
                    tc_map.insert("type".to_string(), serde_json::Value::String(call_type.clone()));
                }

                if let Some(ref function) = tc.function {
                    let mut func_map = serde_json::Map::new();
                    if let Some(ref name) = function.name {
                        func_map.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    }
                    if let Some(ref arguments) = function.arguments {
                        func_map.insert("arguments".to_string(), serde_json::Value::String(arguments.clone()));
                    }
                    tc_map.insert("function".to_string(), serde_json::Value::Object(func_map));
                }

                serde_json::Value::Object(tc_map)
            })
            .collect();

        delta.insert("tool_calls".to_string(), serde_json::Value::Array(tool_calls_json));
    }

    let mut choice = serde_json::Map::new();
    choice.insert("delta".to_string(), serde_json::Value::Object(delta));
    choice.insert("finish_reason".to_string(), serde_json::Value::Null);

    if let Some(ref finish_reason) = chunk.finish_reason {
        choice.insert("finish_reason".to_string(), serde_json::Value::String(finish_reason.clone()));
    }

    let mut response = serde_json::Map::new();
    response.insert("choices".to_string(), serde_json::Value::Array(vec![serde_json::Value::Object(choice)]));

    Event::default().data(serde_json::to_string(&response).unwrap())
}
