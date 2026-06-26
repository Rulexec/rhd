use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use rhd_api::{ErrorCode, WsEvent, WsRequest, WsResponse};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use crate::daemon::DaemonState;
use crate::log::read_finished_scenarios;

pub async fn run_ws_server(
    addr: std::net::SocketAddr,
    state: Arc<DaemonState>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("WebSocket listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_ws_connection(stream, state).await {
                eprintln!("WebSocket connection error from {}: {}", peer_addr, err);
            }
        });
    }
}

async fn handle_ws_connection(
    stream: tokio::net::TcpStream,
    state: Arc<DaemonState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    let mut events_rx = state.execution_tracker.subscribe();

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = handle_ws_message(&text, &state);
                        let response_text = serde_json::to_string(&response)?;
                        write.send(Message::Text(response_text)).await?;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => return Err(Box::new(e)),
                    _ => {}
                }
            }
            event = events_rx.recv() => {
                match event {
                    Ok(exec_event) => {
                        let ws_event = WsEvent::new(
                            &format!("{:?}", exec_event.event).to_lowercase(),
                            serde_json::to_value(&exec_event.data)?,
                        );
                        let event_text = serde_json::to_string(&ws_event)?;
                        write.send(Message::Text(event_text)).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    Ok(())
}

fn handle_ws_message(text: &str, state: &Arc<DaemonState>) -> WsResponse {
    let request: WsRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return WsResponse::error(
                "unknown".to_string(),
                ErrorCode::InvalidRequest,
                format!("invalid JSON: {}", e),
            );
        }
    };

    match request {
        WsRequest::RunScenario { id, name, cwd, model_aliases } => {
            handle_run_scenario(id, name, cwd, model_aliases, state)
        }
        WsRequest::Subscribe { id } => handle_subscribe(id, state),
        WsRequest::GetFinishedScenarios { id, last_id } => handle_get_finished(id, last_id, state),
        WsRequest::AbortScenario { id, execution_id } => handle_abort_scenario(id, execution_id, state),
    }
}

fn handle_run_scenario(
    id: String,
    name: String,
    cwd: String,
    model_aliases: Vec<(String, String)>,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let scenario = match state.scenarios.get(&name) {
        Some(s) => s,
        None => {
            return WsResponse::error(
                id,
                ErrorCode::UnknownScenario,
                format!("unknown scenario: {}", name),
            );
        }
    };

    let log_file = state.logs.as_ref().and_then(|logs_dir| {
        crate::log::create_log_dir(logs_dir, &name)
            .and_then(|dir| crate::log::open_log_file(&dir))
            .ok()
    });
    let mut sink = crate::log::LogSink::new(log_file);

    let handle = state.execution_tracker.start(name.clone());

    let result = tokio::runtime::Handle::current().block_on(crate::scenario::execute_scenario(
        scenario,
        &name,
        &state.models,
        &state.mcp_configs,
        &state.mcp_cache,
        state.default_model.as_deref(),
        &mut sink,
        &cwd,
        Some(handle.clone()),
        &model_aliases,
    ));

    let status = match &result {
        Ok(_) => rhd_api::ScenarioStatus::Success,
        Err(crate::scenario::ExecuteError::Aborted) => {
            sink.log_aborted();
            rhd_api::ScenarioStatus::Aborted
        }
        Err(_) => rhd_api::ScenarioStatus::Error,
    };

    let finished = handle.finished(status);

    match result {
        Ok(output) => {
            let data = serde_json::json!({
                "output": output.outputs.join("\n"),
                "executionId": finished.id,
            });
            WsResponse::success(id, data)
        }
        Err(crate::scenario::ExecuteError::Aborted) => WsResponse::error(
            id,
            ErrorCode::ScenarioAborted,
            "scenario aborted".to_string(),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::ScenarioExecutionFailed,
            err.to_string(),
        ),
    }
}

fn handle_subscribe(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let active = state.execution_tracker.get_active_executions();
    let data = serde_json::json!({
        "activeExecutions": active.iter().map(|e| serde_json::json!({
            "id": e.id,
            "scenarioName": e.scenario_name,
            "startedAt": e.started_at,
        })).collect::<Vec<_>>(),
    });
    WsResponse::success(id, data)
}

fn handle_get_finished(id: String, last_id: Option<u64>, state: &Arc<DaemonState>) -> WsResponse {
    let scenarios = match state.logs.as_ref() {
        Some(logs_dir) => match read_finished_scenarios(logs_dir) {
            Ok(s) => s,
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    };

    let filtered = match last_id {
        Some(last) => scenarios.into_iter().filter(|s| s.id > last).collect(),
        None => scenarios,
    };

    let data = serde_json::to_value(filtered).unwrap_or(serde_json::json!([]));
    WsResponse::success(id, data)
}

fn handle_abort_scenario(id: String, execution_id: u64, state: &Arc<DaemonState>) -> WsResponse {
    state.execution_tracker.abort(execution_id);
    WsResponse::success(id, serde_json::json!({ "aborted": true }))
}
