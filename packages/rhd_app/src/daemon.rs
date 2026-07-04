use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rhd_ai::config::ModelConfig;
use rhd_db::{ChatDb, ScenarioDb};
use rhd_mcp_client::McpConfig;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::broadcast;

use crate::chat::{ChatEvent, ChatManager};
use crate::execution::ExecutionTracker;
use crate::ipc::protocol::{read_message, write_message, IpcRequest, IpcResponse};
use crate::log::{create_log_dir, open_log_file, LogSink};
use crate::mcp_cache::McpServerCache;
use crate::scenario::{execute_scenario, Scenario};

pub struct DaemonState {
    pub scenarios: HashMap<String, Scenario>,
    pub models: HashMap<String, ModelConfig>,
    pub mcp_configs: HashMap<String, McpConfig>,
    pub mcp_cache: McpServerCache,
    pub default_model: Option<String>,
    pub logs: Option<std::path::PathBuf>,
    pub execution_tracker: Arc<ExecutionTracker>,
    pub chat_db: Arc<ChatDb>,
    pub chat_manager: Arc<ChatManager>,
    pub chat_event_sender: broadcast::Sender<ChatEvent>,
    pub frontend_alive: Arc<AtomicBool>,
    pub never_fail: bool,
    pub ws_port: Option<u16>,
}

impl DaemonState {
    pub fn is_frontend_alive(&self) -> bool {
        self.frontend_alive.load(Ordering::Relaxed)
    }
}

pub async fn run_daemon(
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    mcp_configs: HashMap<String, McpConfig>,
    default_model: Option<String>,
    logs: Option<std::path::PathBuf>,
    socket_path: &Path,
    ws_port: Option<u16>,
    db_path: &str,
    never_fail: bool,
) -> std::io::Result<()> {
    let sock_path = socket_path;
    if let Err(err) = std::fs::remove_file(sock_path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err);
        }
    }

    let std_listener = std::os::unix::net::UnixListener::bind(sock_path)?;
    std_listener.set_nonblocking(true)?;
    let listener = UnixListener::from_std(std_listener)?;

    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Arc::new(ScenarioDb::new(db_path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?);
    let execution_tracker = Arc::new(ExecutionTracker::new(db));

    let db_path_obj = std::path::Path::new(db_path);
    let chat_db_path = format!("{}/chats.db", db_path_obj.parent().unwrap_or(Path::new(".")).display());
    let chat_db = Arc::new(ChatDb::new(&chat_db_path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?);
    let chat_manager = Arc::new(ChatManager::new(chat_db.clone()));
    let (chat_event_sender, _) = broadcast::channel(100);

    let frontend_alive = Arc::new(AtomicBool::new(false));
    let state = Arc::new(DaemonState {
        scenarios,
        models,
        mcp_configs,
        mcp_cache: McpServerCache::new(),
        default_model,
        logs,
        execution_tracker: execution_tracker.clone(),
        chat_db,
        chat_manager,
        chat_event_sender,
        frontend_alive,
        never_fail,
        ws_port,
    });

    let scenario_names: Vec<&String> = state.scenarios.keys().collect();
    if scenario_names.is_empty() {
        eprintln!("no scenarios loaded");
    } else {
        eprintln!(
            "loaded {} scenario{}: {}",
            scenario_names.len(),
            if scenario_names.len() == 1 { "" } else { "s" },
            scenario_names.into_iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    println!("listening on {}", sock_path.display());

    if let Some(port) = ws_port {
        let ws_state = state.clone();
        let ws_addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        tokio::spawn(async move {
            if let Err(err) = crate::ws::run_ws_server(ws_addr, ws_state).await {
                eprintln!("WebSocket server error: {}", err);
            }
        });
    }

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, state).await;
                        });
                    }
                    Err(err) => {
                        eprintln!("accept error: {err}");
                    }
                }
            }
            _ = sigterm.recv() => {
                eprintln!("received SIGTERM, shutting down");
                break;
            }
            _ = sigint.recv() => {
                eprintln!("received SIGINT, shutting down");
                break;
            }
        }
    }

    let _ = std::fs::remove_file(sock_path);
    Ok(())
}

async fn handle_connection(stream: tokio::net::UnixStream, state: Arc<DaemonState>) {
    let std_stream = match stream.into_std() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to convert stream: {err}");
            return;
        }
    };
    if let Err(err) = std_stream.set_nonblocking(false) {
        eprintln!("failed to set stream to blocking: {err}");
        return;
    }
    
    let connection_executions: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let state_clone = state.clone();
    let execs_clone = connection_executions.clone();
    
    tokio::task::spawn_blocking(move || {
        if let Err(err) = run_connection(std_stream, &state_clone, execs_clone) {
            eprintln!("connection error: {err}");
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

fn handle_request(
    request: IpcRequest,
    state: &DaemonState,
    connection_executions: Arc<Mutex<Vec<u64>>>,
) -> Vec<IpcResponse> {
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
            let _ = state.chat_event_sender.send(crate::chat::ChatEvent::DevNotification {
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
    }
}

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

    let scenario = match state.scenarios.get(&name) {
        Some(s) => s.clone(),
        None => {
            let mut sink = LogSink::new(log_file);
            sink.log(&name, "unknown scenario", "");
            return vec![IpcResponse::Error {
                message: format!("unknown scenario: {name}"),
            }];
        }
    };

    let handle = state.execution_tracker.start(name.clone());
    let execution_id = handle.id();
    
    // Track this execution for cleanup on disconnect
    connection_executions.lock().unwrap().push(execution_id);
    
    let exec_config = crate::scenario::ExecutionConfig {
        frontend_alive: state.is_frontend_alive(),
        never_fail: state.never_fail,
    };
    
    let models = state.models.clone();
    let mcp_configs = state.mcp_configs.clone();
    let mcp_cache = state.mcp_cache.clone();
    let default_model = state.default_model.clone();
    
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
        let model_config = state.models.values().next();
        let meta = build_scenario_meta(&name, &finished, model_config, status);
        if let Err(err) = crate::log::write_meta_json(&dir, &meta) {
            eprintln!("failed to write meta.json: {}", err);
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
