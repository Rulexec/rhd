use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::mock_server::{SharedRequests, SharedResponse, StreamChunkSender, SharedAutoStream};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectScenarioMetaRequest {
    pub id: u64,
    pub scenario: String,
    pub status: String,
    pub started: String,
    pub finished: String,
    pub duration_ms: u64,
}

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
    logs_dir: Option<std::path::PathBuf>,
    socket_path: Option<std::path::PathBuf>,
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

#[derive(Deserialize)]
struct TestScenarioEventRequest {
    id: u64,
    name: String,
}

async fn test_scenario_started(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<TestScenarioEventRequest>,
) -> Json<ControlResponse> {
    let Some(socket_path) = &state.socket_path else {
        return error_response("Socket path not configured");
    };

    match send_ipc_test_event(socket_path, "TestScenarioStarted", body.id, &body.name).await {
        Ok(_) => ok_response(),
        Err(e) => error_response(&format!("Failed to send IPC message: {}", e)),
    }
}

async fn test_scenario_finished(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<TestScenarioEventRequest>,
) -> Json<ControlResponse> {
    let Some(socket_path) = &state.socket_path else {
        return error_response("Socket path not configured");
    };

    match send_ipc_test_event(socket_path, "TestScenarioFinished", body.id, &body.name).await {
        Ok(_) => ok_response(),
        Err(e) => error_response(&format!("Failed to send IPC message: {}", e)),
    }
}

async fn send_ipc_test_event(
    socket_path: &std::path::Path,
    event_type: &str,
    id: u64,
    name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rhd_app::ipc::protocol::{IpcRequest, write_message};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)?;

    let request = match event_type {
        "TestScenarioStarted" => IpcRequest::TestScenarioStarted {
            id,
            name: name.to_string(),
        },
        "TestScenarioFinished" => IpcRequest::TestScenarioFinished {
            id,
            name: name.to_string(),
        },
        _ => return Err("Unknown event type".into()),
    };

    write_message(&mut stream, &request)?;
    Ok(())
}

async fn inject_finished_scenario(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<InjectScenarioMetaRequest>,
) -> Json<ControlResponse> {
    let Some(logs_dir) = &state.logs_dir else {
        return error_response("Logs directory not configured");
    };

    let scenario_dir_name = format!("{}-{}", body.scenario, body.id);
    let scenario_dir = logs_dir.join(&scenario_dir_name);

    if let Err(e) = std::fs::create_dir_all(&scenario_dir) {
        return error_response(&format!("Failed to create scenario directory: {}", e));
    }

    let meta_json = serde_json::json!({
        "id": body.id,
        "scenario": body.scenario,
        "status": body.status,
        "started": body.started,
        "finished": body.finished,
        "durationMs": body.duration_ms,
        "steps": []
    });

    let meta_path = scenario_dir.join("meta.json");
    match std::fs::write(&meta_path, serde_json::to_string_pretty(&meta_json).unwrap()) {
        Ok(_) => ok_response(),
        Err(e) => error_response(&format!("Failed to write meta.json: {}", e)),
    }
}

pub async fn start_control_server(
    port: u16,
    response: SharedResponse,
    requests: SharedRequests,
    daemon_ready: Arc<Mutex<bool>>,
    stream_sender: StreamChunkSender,
    auto_stream: SharedAutoStream,
    logs_dir: Option<std::path::PathBuf>,
    socket_path: Option<std::path::PathBuf>,
) {
    let state = Arc::new(ControlState {
        response,
        requests,
        daemon_ready,
        stream_sender,
        auto_stream,
        logs_dir,
        socket_path,
    });

    let app = Router::new()
        .route("/mock-response", post(set_mock_response))
        .route("/status", get(get_status))
        .route("/requests", get(get_requests))
        .route("/stream-chunk", post(emit_stream_chunk))
        .route("/stream-finish", post(finish_stream))
        .route("/stream-ready", get(is_stream_ready))
        .route("/set-auto-stream", post(set_auto_stream))
        .route("/scenarios/finished", post(inject_finished_scenario))
        .route("/test/scenario-started", post(test_scenario_started))
        .route("/test/scenario-finished", post(test_scenario_finished))
        .with_state(state);

    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
}
