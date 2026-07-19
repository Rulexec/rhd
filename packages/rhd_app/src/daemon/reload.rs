use std::collections::HashMap;
use std::sync::Arc;

use rhd_mcp_client::McpConfig;
use tracing::instrument;

use crate::ipc::protocol::IpcResponse;
use crate::project_manager::ProjectManager;

use super::DaemonState;

#[instrument(level = "info", skip(state))]
pub async fn handle_reload(state: &DaemonState) -> Vec<IpcResponse> {
    // Acquire write lock on reload_lock (blocks until all executions/chats release read locks)
    let _reload_guard = state.reload_lock.write().await;
    
    // Get config paths
    let config_paths = {
        let inner = state.inner.read().await;
        inner.config_paths.clone()
    };
    
    // Load credentials
    let credentials = if let Some(cred_path) = &config_paths.credentials_config {
        if cred_path.exists() {
            match crate::credentials::load_credentials(cred_path) {
                Ok(c) => c,
                Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
            }
        } else {
            return vec![IpcResponse::Error {
                message: format!("credentials file not found: {}", cred_path.display())
            }];
        }
    } else {
        HashMap::new()
    };
    
    // Load new configs
    let new_scenarios = match crate::scenario::load_scenarios_dir(&config_paths.scenarios_dir) {
        Ok(s) => s,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_models = match rhd_ai::config::load_models(&config_paths.models_dir, &credentials) {
        Ok(m) => m,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_mcp_configs = match crate::mcp_loader::load_mcp_dir(&config_paths.mcp_dir) {
        Ok(m) => m,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_projects = match crate::project_loader::load_projects(&config_paths.projects_dir) {
        Ok(p) => p,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    
    // Diff MCP configs
    let (mcp_to_restart, mcp_to_stop) = {
        let inner = state.inner.read().await;
        diff_mcp_configs(&inner.mcp_configs, &new_mcp_configs)
    };
    
    // Stop removed MCPs
    let stopped_count = state.mcp_cache.stop_specific(&mcp_to_stop).await;
    
    // Restart changed MCPs
    let restarted_count = state.mcp_cache.restart_specific(&mcp_to_restart).await;
    
    // Update state
    {
        let mut inner = state.inner.write().await;
        inner.scenarios = new_scenarios;
        inner.models = new_models;
        inner.mcp_configs = new_mcp_configs.clone();
        inner.project_manager = Arc::new(ProjectManager::new(new_projects, new_mcp_configs, state.mcp_cache.clone()));
    }
    
    // Return stats
    let inner = state.inner.read().await;
    vec![IpcResponse::Reloaded {
        scenarios_reloaded: inner.scenarios.len(),
        models_reloaded: inner.models.len(),
        mcp_restarted: restarted_count,
        mcp_stopped: stopped_count,
        projects_reloaded: inner.project_manager.list_projects().len(),
    }]
}

fn diff_mcp_configs(
    old: &HashMap<String, McpConfig>,
    new: &HashMap<String, McpConfig>,
) -> (Vec<String>, Vec<String>) {
    let mut to_restart = Vec::new();
    let mut to_stop = Vec::new();
    
    // Find changed or new configs
    for (name, new_config) in new {
        let new_key = mcp_cache_key(new_config);
        
        match old.get(name) {
            Some(old_config) => {
                let old_key = mcp_cache_key(old_config);
                if old_key != new_key {
                    to_restart.push(old_key);
                }
            }
            None => {
                // New config, will be spawned on first use
            }
        }
    }
    
    // Find removed configs
    for (name, old_config) in old {
        if !new.contains_key(name) {
            to_stop.push(mcp_cache_key(old_config));
        }
    }
    
    (to_restart, to_stop)
}

fn mcp_cache_key(config: &McpConfig) -> String {
    format!(
        "{}:{}:{}",
        config.cmd.as_deref().unwrap_or(""),
        config.args.join(","),
        config.cwd.as_deref().unwrap_or("")
    )
}
