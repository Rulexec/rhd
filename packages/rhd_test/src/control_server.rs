use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::mock_server::{SharedRequests, SharedResponse};

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
}

async fn set_mock_response(
    State(state): State<Arc<ControlState>>,
    Json(body): Json<MockResponseRequest>,
) {
    *state.response.lock().unwrap() = body.content;
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

pub async fn start_control_server(
    port: u16,
    response: SharedResponse,
    requests: SharedRequests,
    daemon_ready: Arc<Mutex<bool>>,
) {
    let state = Arc::new(ControlState {
        response,
        requests,
        daemon_ready,
    });

    let app = Router::new()
        .route("/mock-response", post(set_mock_response))
        .route("/status", get(get_status))
        .route("/requests", get(get_requests))
        .with_state(state);

    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
}
