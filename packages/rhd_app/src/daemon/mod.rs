mod ipc;
mod reload;
mod run;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_mcp_client::McpConfig;
use tokio::sync::{broadcast, RwLock};

use rhd_chat::{ChatEvent, ChatManager};
use crate::execution::ExecutionTracker;
use crate::mcp_cache::McpServerCache;
use crate::project_manager::ProjectManager;
use crate::scenario::Scenario;
use crate::template_loader::TemplateLoader;

#[derive(Clone)]
pub struct ResolvedConfigPaths {
    pub models_dir: PathBuf,
    pub scenarios_dir: PathBuf,
    pub mcp_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub credentials_config: Option<PathBuf>,
}

pub struct ReloadableInner {
    pub scenarios: HashMap<String, Scenario>,
    pub models: HashMap<String, ModelConfig>,
    pub mcp_configs: HashMap<String, McpConfig>,
    pub default_model: Option<String>,
    pub project_manager: Arc<ProjectManager>,
    pub config_paths: ResolvedConfigPaths,
}

pub struct DaemonState {
    pub inner: RwLock<ReloadableInner>,
    pub mcp_cache: Arc<McpServerCache>,
    pub logs: Option<std::path::PathBuf>,
    pub execution_tracker: Arc<ExecutionTracker>,
    pub chat_manager: Arc<ChatManager<ProjectManager>>,
    pub chat_event_sender: broadcast::Sender<ChatEvent>,
    pub frontend_alive: Arc<AtomicBool>,
    pub never_fail: bool,
    pub reload_lock: RwLock<()>,
    pub template_loader: Arc<TemplateLoader>,
}

impl DaemonState {
    pub fn is_frontend_alive(&self) -> bool {
        self.frontend_alive.load(Ordering::Relaxed)
    }
}

pub use run::run_daemon;
