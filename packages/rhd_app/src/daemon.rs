use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};

use crate::ipc::protocol::{read_message, write_message, IpcRequest, IpcResponse};
use crate::scenario::{execute_scenario, Scenario};

struct DaemonState {
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    default_model: Option<String>,
}

pub async fn run_daemon(
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    default_model: Option<String>,
) -> std::io::Result<()> {
    let sock_path = Path::new("rhd.sock");
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
        default_model,
    });

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    eprintln!("listening on {}", sock_path.display());

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
        IpcRequest::RunScenario { name } => {
            let scenario = match state.scenarios.get(&name) {
                Some(s) => s,
                None => {
                    return IpcResponse::Error {
                        message: format!("unknown scenario: {name}"),
                    }
                }
            };
            let result = tokio::runtime::Handle::current().block_on(execute_scenario(
                scenario,
                &state.models,
                state.default_model.as_deref(),
            ));
            match result {
                Ok(output) => IpcResponse::Success {
                    output: output.outputs.join("\n"),
                },
                Err(err) => IpcResponse::Error {
                    message: err.to_string(),
                },
            }
        }
    }
}
