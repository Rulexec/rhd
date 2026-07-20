use std::sync::{Arc, Mutex};

use rhd_ai::config::ModelConfig;
use tracing::{error, instrument};

use crate::ipc::protocol::{read_message, write_message, IpcRequest, IpcResponse};
use crate::log::{create_log_dir, open_log_file, LogSink};
use crate::scenario::execute_scenario;

use super::reload::handle_reload;
use super::DaemonState;

#[instrument(level = "debug", skip(state))]
pub async fn handle_connection(stream: tokio::net::UnixStream, state: Arc<DaemonState>) {
    let std_stream = match stream.into_std() {
        Ok(s) => s,
        Err(err) => {
            error!(%err, "failed to convert stream");
            return;
        }
    };
    if let Err(err) = std_stream.set_nonblocking(false) {
        error!(%err, "failed to set stream to blocking");
        return;
    }
    
    let connection_executions: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let state_clone = state.clone();
    let execs_clone = connection_executions.clone();
    
    tokio::task::spawn_blocking(move || {
        if let Err(err) = run_connection(std_stream, &state_clone, execs_clone) {
            error!(%err, "connection error");
        }
    })
    .await
    .ok();
    
    // Connection closed, abort any paused executions
    let execs = connection_executions.lock().unwrap();
    for exec_id in execs.iter() {
        state.execution_tracker.abort_paused(*exec_id);
    }
}

fn run_connection(
    mut stream: std::os::unix::net::UnixStream,
    state: &DaemonState,
    connection_executions: Arc<Mutex<Vec<u64>>>,
) -> std::io::Result<()> {
    loop {
        let request = match read_message::<IpcRequest>(&mut stream) {
            Ok(r) => r,
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err),
        };
        let responses = handle_request(request, state, connection_executions.clone());
        for response in responses {
            write_message(&mut stream, &response)?;
        }
    }
}

#[instrument(level = "debug", skip(state, connection_executions), fields(request_type))]
fn handle_request(
    request: IpcRequest,
    state: &DaemonState,
    connection_executions: Arc<Mutex<Vec<u64>>>,
) -> Vec<IpcResponse> {
    let request_type = match &request {
        IpcRequest::RunScenario { name, .. } => format!("RunScenario({})", name),
        IpcRequest::DaemonNotification => "DaemonNotification".to_string(),
        IpcRequest::FrontendNotification => "FrontendNotification".to_string(),
        IpcRequest::TestScenarioStarted { id, name } => format!("TestScenarioStarted({}, {})", id, name),
        IpcRequest::TestScenarioFinished { id, name } => format!("TestScenarioFinished({}, {})", id, name),
        IpcRequest::Reload => "Reload".to_string(),
    };
    tracing::Span::current().record("request_type", request_type.as_str());
    match request {
        IpcRequest::RunScenario { name, cwd, model_aliases } => {
            handle_run_scenario(name, cwd, model_aliases, state, connection_executions)
        }
        IpcRequest::DaemonNotification => {
            let url = "http://localhost:5173";
            crate::notifications::send_notification(
                "RHD Daemon",
                "Test notification from daemon",
                Some(url),
            );
            vec![IpcResponse::Ok]
        }
        IpcRequest::FrontendNotification => {
            let _ = state.chat_event_sender.send(rhd_chat::ChatEvent::DevNotification {
                title: "RHD Test".to_string(),
                message: "This is a test notification from rhd dev frontend-notification".to_string(),
            });
            vec![IpcResponse::Ok]
        }
        IpcRequest::TestScenarioStarted { id, name } => {
            state.execution_tracker.emit_test_scenario_started(id, name);
            vec![IpcResponse::Ok]
        }
        IpcRequest::TestScenarioFinished { id, name } => {
            state.execution_tracker.emit_test_scenario_finished(id, name);
            vec![IpcResponse::Ok]
        }
        IpcRequest::Reload => {
            tokio::runtime::Handle::current().block_on(handle_reload(state))
        }
    }
}

#[instrument(level = "info", skip(state, connection_executions, model_aliases), fields(scenario = %name, cwd = %cwd))]
fn handle_run_scenario(
    name: String,
    cwd: String,
    model_aliases: Vec<(String, String)>,
    state: &DaemonState,
    connection_executions: Arc<Mutex<Vec<u64>>>,
) -> Vec<IpcResponse> {
    let log_dir = state.logs.as_ref().and_then(|logs_dir| {
        create_log_dir(logs_dir, &name).ok()
    });
    let log_file = log_dir.as_ref().and_then(|dir| open_log_file(dir).ok());

    // Acquire reload_lock read — blocks silently if reload holds write lock
    let _reload_guard = state.reload_lock.blocking_read();
    let inner_guard = state.inner.blocking_read();
    let scenario = match inner_guard.scenarios.get(&name) {
        Some(s) => s.clone(),
        None => {
            let mut sink = LogSink::new(log_file);
            sink.log(&name, "unknown scenario", "");
            return vec![IpcResponse::Error {
                message: format!("unknown scenario: {name}"),
            }];
        }
    };
    let models = inner_guard.models.clone();
    let mcp_configs = inner_guard.mcp_configs.clone();
    let default_model = inner_guard.default_model.clone();
    drop(inner_guard);

    let handle = state.execution_tracker.start(name.clone());
    let execution_id = handle.id();
    
    // Track this execution for cleanup on disconnect
    connection_executions.lock().unwrap().push(execution_id);
    
    let exec_config = crate::scenario::ExecutionConfig {
        frontend_alive: state.is_frontend_alive(),
        never_fail: state.never_fail,
    };
    
    let mcp_cache = state.mcp_cache.clone();
    
    let mut pause_rx = state.execution_tracker.subscribe_pause();
    
    let name_for_async = name.clone();
    let handle_for_async = handle.clone();
    
    let execution_future = async move {
        let mut sink = LogSink::new(log_file);
        execute_scenario(
            &scenario,
            &name_for_async,
            &models,
            &mcp_configs,
            &mcp_cache,
            default_model.as_deref(),
            &mut sink,
            &cwd,
            Some(handle_for_async),
            &model_aliases,
            &exec_config,
        ).await
    };
    
    let mut responses = Vec::new();
    
    let exec_result = tokio::runtime::Handle::current().block_on(async {
        let mut execution_task = tokio::spawn(execution_future);
        
        // Wait for either pause or completion
        let pause_detected = tokio::select! {
            result = &mut execution_task => {
                // Execution completed without pause
                return result.unwrap();
            }
            pause_notification = pause_rx.recv() => {
                if let Ok(notification) = pause_notification {
                    notification.execution_id == execution_id
                } else {
                    false
                }
            }
        };
        
        if pause_detected {
            let paused_state = state.execution_tracker.get_paused_state(execution_id);
            if let Some(paused) = paused_state {
                // Send desktop notification only if no frontend is alive
                if !state.is_frontend_alive() {
                    let notification_title = format!("Scenario '{}' paused", name);
                    let notification_message = format!(
                        "Paused at step '{}': {}",
                        paused.step_name, paused.error
                    );
                    let url = Some("http://localhost:5173");
                    crate::notifications::send_notification(
                        &notification_title,
                        &notification_message,
                        url,
                    );
                }
                
                // Add Paused response to responses list
                responses.push(IpcResponse::Paused {
                    error: paused.error,
                    step: paused.step_name,
                });
                
                // Wait for execution to complete (after retry or abort)
                // Client is waiting for final response
            }
        }
        
        // Wait for execution to complete
        execution_task.await.unwrap()
    });
    
    {
        let status = match &exec_result {
            Ok(_) => rhd_api::ScenarioStatus::Success,
            Err(crate::scenario::ExecuteError::Aborted) => {
                rhd_api::ScenarioStatus::Aborted
            }
            Err(_) => rhd_api::ScenarioStatus::Error,
        };

        let finished = handle.finished(status);

        if let Some(dir) = log_dir {
            let inner_guard = state.inner.blocking_read();
            let model_config = inner_guard.models.values().next();
            let meta = build_scenario_meta(&name, &finished, model_config, status);
            if let Err(err) = crate::log::write_meta_json(&dir, &meta) {
                error!(%err, "failed to write meta.json");
            }
        }

        let mut sink = LogSink::new(None);
        let final_response = match exec_result {
            Ok(output) => IpcResponse::Success {
                output: output.outputs.join("\n"),
            },
            Err(crate::scenario::ExecuteError::Aborted) => {
                sink.log_aborted();
                IpcResponse::Aborted
            },
            Err(err) => {
                sink.log("error", "scenario execution error", &err.to_string());
                IpcResponse::Error {
                    message: err.to_string(),
                }
            },
        };
        
        responses.push(final_response);
        responses
    }
}

fn build_scenario_meta(
    name: &str,
    finished: &crate::execution::FinishedExecution,
    model_config: Option<&ModelConfig>,
    status: rhd_api::ScenarioStatus,
) -> rhd_api::ScenarioMeta {
    let mut step_timings = finished.step_timings.clone();

    if let Some(config) = model_config {
        for step in &mut step_timings {
            if let Some(tokens) = &step.tokens {
                step.cost = rhd_api::calculate_cost(
                    tokens,
                    config.input_token_price,
                    config.output_token_price,
                    config.price_tiers.as_deref(),
                );
            }
        }
    }

    let total_cost = if let Some(config) = model_config {
        finished.token_usage.as_ref().and_then(|tokens| {
            rhd_api::calculate_cost(
                tokens,
                config.input_token_price,
                config.output_token_price,
                config.price_tiers.as_deref(),
            )
        })
    } else {
        None
    };

    rhd_api::ScenarioMeta {
        id: finished.id,
        scenario: name.to_string(),
        status,
        started: finished.started_at,
        finished: finished.finished_at,
        duration_ms: finished.duration_ms,
        tokens: finished.token_usage.clone(),
        cost: total_cost,
        steps: step_timings,
    }
}
