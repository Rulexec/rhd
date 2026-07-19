use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_db::{ChatDb, ScenarioDb};
use rhd_mcp_client::McpConfig;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn, error, instrument};

use rhd_chat::ChatManager;
use crate::execution::ExecutionTracker;
use crate::template_loader::TemplateLoader;
use crate::mcp_cache::McpServerCache;
use crate::project_manager::ProjectManager;
use crate::scenario::Scenario;

use super::{DaemonState, ReloadableInner, ResolvedConfigPaths};
use super::ipc::handle_connection;

#[instrument(level = "info", skip(scenarios, models, mcp_configs, logs, log_chats, project_manager, config_paths), fields(socket_path = %socket_path.display(), ws_port = ws_port, db_path = %db_path))]
pub async fn run_daemon(
    scenarios: HashMap<String, Scenario>,
    models: HashMap<String, ModelConfig>,
    mcp_configs: HashMap<String, McpConfig>,
    default_model: Option<String>,
    logs: Option<std::path::PathBuf>,
    log_chats: Option<std::path::PathBuf>,
    log_chats_raw: bool,
    socket_path: &Path,
    ws_port: Option<u16>,
    db_path: &str,
    never_fail: bool,
    project_manager: Arc<ProjectManager>,
    config_paths: ResolvedConfigPaths,
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
    let chat_manager = Arc::new(ChatManager::new(chat_db.clone(), project_manager.clone(), log_chats, log_chats_raw));
    let (chat_event_sender, _) = broadcast::channel(100);

    let template_loader = Arc::new(TemplateLoader::new());

    let frontend_alive = Arc::new(AtomicBool::new(false));
    let inner = ReloadableInner {
        scenarios,
        models,
        mcp_configs,
        default_model,
        project_manager,
        config_paths,
    };
    let state = Arc::new(DaemonState {
        inner: RwLock::new(inner),
        mcp_cache: Arc::new(McpServerCache::new()),
        logs,
        execution_tracker: execution_tracker.clone(),
        chat_manager,
        chat_event_sender,
        frontend_alive,
        never_fail,
        reload_lock: RwLock::new(()),
        template_loader,
    });

    let inner_guard = state.inner.read().await;
    let scenario_names: Vec<&String> = inner_guard.scenarios.keys().collect();
    if scenario_names.is_empty() {
        warn!("no scenarios loaded");
    } else {
        let count = scenario_names.len();
        let names = scenario_names.into_iter().cloned().collect::<Vec<_>>().join(", ");
        info!(count, %names, "loaded scenarios");
    }
    let mcp_names: Vec<&String> = inner_guard.mcp_configs.keys().collect();
    if mcp_names.is_empty() {
        warn!("no MCP configs loaded");
    } else {
        let count = mcp_names.len();
        let names = mcp_names.into_iter().cloned().collect::<Vec<_>>().join(", ");
        info!(count, %names, "loaded MCP configs");
    }
    drop(inner_guard);

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    info!(path = %sock_path.display(), "daemon listening");

    if let Some(port) = ws_port {
        let ws_state = state.clone();
        let ws_addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        println!("WebSocket server started on port {}", port);
        tokio::spawn(async move {
            if let Err(err) = crate::ws::run_ws_server(ws_addr, ws_state).await {
                error!(%err, "WebSocket server error");
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
                        error!(%err, "accept error");
                    }
                }
            }
            _ = sigterm.recv() => {
                info!("received SIGTERM, shutting down");
                break;
            }
            _ = sigint.recv() => {
                info!("received SIGINT, shutting down");
                break;
            }
        }
    }

    let _ = std::fs::remove_file(sock_path);
    Ok(())
}
