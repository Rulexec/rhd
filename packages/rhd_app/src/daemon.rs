use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_db::ScenarioDb;
use rhd_mcp_client::McpConfig;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};

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

    let state = Arc::new(DaemonState {
        scenarios,
        models,
        mcp_configs,
        mcp_cache: McpServerCache::new(),
        default_model,
        logs,
        execution_tracker: execution_tracker.clone(),
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
    tokio::task::spawn_blocking(move || {
        if let Err(err) = run_connection(std_stream, &state) {
            eprintln!("connection error: {err}");
        }
    })
    .await
    .ok();
}

fn run_connection(
    mut stream: std::os::unix::net::UnixStream,
    state: &DaemonState,
) -> std::io::Result<()> {
    loop {
        let request = match read_message::<IpcRequest>(&mut stream) {
            Ok(r) => r,
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err),
        };
        let response = handle_request(request, state);
        write_message(&mut stream, &response)?;
    }
}

fn handle_request(request: IpcRequest, state: &DaemonState) -> IpcResponse {
    match request {
        IpcRequest::RunScenario { name, cwd } => {
            let log_dir = state.logs.as_ref().and_then(|logs_dir| {
                create_log_dir(logs_dir, &name).ok()
            });
            let log_file = log_dir.as_ref().and_then(|dir| open_log_file(dir).ok());
            let mut sink = LogSink::new(log_file);

            let scenario = match state.scenarios.get(&name) {
                Some(s) => s,
                None => {
                    sink.log(&name, "unknown scenario", "");
                    return IpcResponse::Error {
                        message: format!("unknown scenario: {name}"),
                    };
                }
            };

            let handle = state.execution_tracker.start(name.clone());

            let result = tokio::runtime::Handle::current().block_on(execute_scenario(
                scenario,
                &name,
                &state.models,
                &state.mcp_configs,
                &state.mcp_cache,
                state.default_model.as_deref(),
                &mut sink,
                &cwd,
                Some(handle.clone()),
            ));

            let finished = handle.finished();

            if let Some(dir) = log_dir {
                let model_config = state.models.values().next();
                let meta = build_scenario_meta(&name, &finished, model_config);
                if let Err(err) = crate::log::write_meta_json(&dir, &meta) {
                    eprintln!("failed to write meta.json: {}", err);
                }
            }

            match result {
                Ok(output) => IpcResponse::Success {
                    output: output.outputs.join("\n"),
                },
                Err(err) => {
                    sink.log("error", "scenario execution error", &err.to_string());
                    IpcResponse::Error {
                        message: err.to_string(),
                    }
                },
            }
        }
    }
}

fn build_scenario_meta(
    name: &str,
    finished: &crate::execution::FinishedExecution,
    model_config: Option<&ModelConfig>,
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
        started: finished.started_at,
        finished: finished.finished_at,
        duration_ms: finished.duration_ms,
        tokens: finished.token_usage.clone(),
        cost: total_cost,
        steps: step_timings,
    }
}
