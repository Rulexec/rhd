use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::mock_server::{SharedRequests, SharedResponse, StreamChunkSender, SharedAutoStream};

#[derive(Deserialize)]
struct MockResponseRequest {
    content: String,
}

#[derive(Serialize)]
struct StatusResponse {
    daemon_ready: bool,
}

#[derive(Serialize)]
struct RequestsResponse {
    requests: Vec<RecordedRequestResponse>,
}

#[derive(Serialize)]
struct RecordedRequestResponse {
    model: String,
    system_content: String,
    user_content: String,
}

struct ControlState {
    response: SharedResponse,
    requests: SharedRequests,
    daemon_ready: Arc<Mutex<bool>>,
    stream_sender: StreamChunkSender,
    auto_stream: SharedAutoStream,
}

#[derive(Serialize)]
struct ControlResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn ok_response() -> Json<ControlResponse> {
    Json(ControlResponse {
        status: "OK".to_string(),
        message: None,
    })
}

fn error_response(msg: &str) -> Json<ControlResponse> {
    Json(ControlResponse {
        status: "ERROR".to_string(),
        message: Some(msg.to_string()),
    })
}

async fn set_mock_response(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<MockResponseRequest>,
) -> Json<ControlResponse> {
    *state.response.lock().unwrap() = body.content;
    *state.auto_stream.lock().unwrap() = true;
    ok_response()
}

async fn get_status(State(state): State<Arc<ControlState>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        daemon_ready: *state.daemon_ready.lock().unwrap(),
    })
}

async fn get_requests(State(state): State<Arc<ControlState>>) -> Json<RequestsResponse> {
    let requests = state.requests.lock().unwrap();
    Json(RequestsResponse {
        requests: requests
            .iter()
            .map(|r| RecordedRequestResponse {
                model: r.model.clone(),
                system_content: r.system_content.clone(),
                user_content: r.user_content.clone(),
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct StreamChunkRequest {
    content: String,
}

async fn emit_stream_chunk(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<StreamChunkRequest>,
) -> Json<ControlResponse> {
    *state.auto_stream.lock().unwrap() = false;
    let sender = state.stream_sender.lock().unwrap().clone();
    if let Some(sender) = sender {
        match sender.send(Some(body.content)).await {
            Ok(_) => ok_response(),
            Err(e) => error_response(&format!("Failed to send chunk: {}", e)),
        }
    } else {
        error_response("No active stream")
    }
}

async fn finish_stream(State(state): State<Arc<ControlState>>) -> Json<ControlResponse> {
    let sender = state.stream_sender.lock().unwrap().clone();
    if let Some(sender) = sender {
        match sender.send(None).await {
            Ok(_) => ok_response(),
            Err(e) => error_response(&format!("Failed to finish stream: {}", e)),
        }
    } else {
        error_response("No active stream")
    }
}

#[derive(Serialize)]
struct StreamReadyResponse {
    ready: bool,
}

async fn is_stream_ready(State(state): State<Arc<ControlState>>) -> Json<StreamReadyResponse> {
    let ready = state.stream_sender.lock().unwrap().is_some();
    Json(StreamReadyResponse { ready })
}

#[derive(Deserialize)]
struct SetAutoStreamRequest {
    enabled: bool,
}

async fn set_auto_stream(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<SetAutoStreamRequest>,
) -> Json<ControlResponse> {
    *state.auto_stream.lock().unwrap() = body.enabled;
    ok_response()
}

pub async fn start_control_server(
    port: u16,
    response: SharedResponse,
    requests: SharedRequests,
    daemon_ready: Arc<Mutex<bool>>,
    stream_sender: StreamChunkSender,
    auto_stream: SharedAutoStream,
) {
    let state = Arc::new(ControlState {
        response,
        requests,
        daemon_ready,
        stream_sender,
        auto_stream,
    });

    let app = Router::new()
        .route("/mock-response", post(set_mock_response))
        .route("/status", get(get_status))
        .route("/requests", get(get_requests))
        .route("/stream-chunk", post(emit_stream_chunk))
        .route("/stream-finish", post(finish_stream))
        .route("/stream-ready", get(is_stream_ready))
        .route("/set-auto-stream", post(set_auto_stream))
        .with_state(state);

    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
}
