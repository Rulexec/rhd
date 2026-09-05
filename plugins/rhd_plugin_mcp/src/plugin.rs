//! Core plugin lifecycle (implemented in Phase 4).

use crate::config::PluginConfig;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("config error: {0}")]
    Config(String),
}

pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    worktree: Option<&str>,
    config: PluginConfig,
) -> Result<(), PluginError> {
    tracing::info!(
        server_url,
        plugin_id,
        worktree = ?worktree,
        servers = config.servers.len(),
        "plugin lifecycle not yet implemented (Phase 4)"
    );
    Ok(())
}
