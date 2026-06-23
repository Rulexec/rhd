use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_mcp_client::McpConfig;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};

use crate::ipc::protocol::{read_message, write_message, IpcRequest, IpcResponse};
use crate::log::{create_log_dir, open_log_file, LogSink};
use crate::mcp_cache::McpServerCache;
use crate::scenario::{execute_scenario, Scenario};

struct DaemonState {
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    mcp_configs: HashMap<String, McpConfig>,
    mcp_cache: McpServerCache,
    default_model: Option<String>,
    logs: Option<std::path::PathBuf>,
}

pub async fn run_daemon(
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    mcp_configs: HashMap<String, McpConfig>,
    default_model: Option<String>,
    logs: Option<std::path::PathBuf>,
    socket_path: &Path,
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

    let state = Arc::new(DaemonState {
        scenarios,
        models,
        mcp_configs,
        mcp_cache: McpServerCache::new(),
        default_model,
        logs,
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
            let log_file = state.logs.as_ref().and_then(|logs_dir| {
                create_log_dir(logs_dir, &name)
                    .and_then(|dir| open_log_file(&dir))
                    .ok()
            });
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
            let result = tokio::runtime::Handle::current().block_on(execute_scenario(
                scenario,
                &name,
                &state.models,
                &state.mcp_configs,
                &state.mcp_cache,
                state.default_model.as_deref(),
                &mut sink,
                &cwd,
            ));
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
