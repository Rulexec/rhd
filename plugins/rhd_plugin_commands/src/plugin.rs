//! Core plugin lifecycle for the slash-commands plugin (Phase 3 stub).
//!
//! Connects and registers with the chat server, then idles. Phase 4 adds:
//! `getPendingAcks` processing, the `ai_completions:preDrainQueue` subscription
//! (see [`crate::PRE_DRAIN_QUEUE_EVENT`]), and the command executor consuming
//! the [`config::CommandRegistry`] passed to [`run_plugin`].

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::RegisterPluginParams;
use rhd_chat_client::ChatClient;

use crate::config;

/// Run the commands plugin.
///
/// Lifecycle (stub):
/// 1. Connect to chat server (with retry)
/// 2. Register as plugin
/// 3. Keep running (executor arrives in Phase 4)
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    _registry: config::CommandRegistry,
) -> Result<(), PluginError> {
    // Connect to chat server
    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );

    tracing::info!("Connected to chat server");

    // Register as plugin
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;

    tracing::info!("Registered as plugin: {}", plugin_id);

    // Keep the plugin running
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),

    #[error("failed to register plugin: {0}")]
    Registration(String),
}
