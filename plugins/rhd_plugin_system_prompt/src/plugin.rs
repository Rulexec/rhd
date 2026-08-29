//! Core plugin lifecycle and main loop.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{GetPendingAcksParams, RegisterPluginParams};
use rhd_chat_client::{ChatClient, ChatMonitor};

use crate::config::{CachedPrompt, PluginConfig};
use crate::system_prompt;

/// Run the system prompt plugin.
///
/// This function implements the complete plugin lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create monitors
/// 5. Subscribe to all chats
/// 6. Startup reconciliation
/// 7. Main loop: check for missing system prompts
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    _config: PluginConfig,
    cached_prompts: Vec<CachedPrompt>,
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

    // Get pending acks
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;

    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

    // Process pending acks
    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        client
            .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    // Create chat monitor
    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );

    tracing::info!("Created chat monitor");

    // Subscribe to all chats
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;

    tracing::info!("Subscribed to all chats");

    // Startup reconciliation
    let injected_count = startup_reconciliation(&client, &chat_monitor, &cached_prompts).await?;
    tracing::info!("Startup reconciliation: injected {} system prompts", injected_count);

    // Main loop
    loop {
        let chat_ids = chat_monitor.get_chat_ids().await;
        tracing::debug!(
            monitored_chats = chat_ids.len(),
            "main loop iteration"
        );

        for chat_id in chat_ids {
            if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
                match system_prompt::process_chat(&client, &chat_state, &cached_prompts).await {
                    Ok(count) if count > 0 => {
                        tracing::info!(
                            chat_id = chat_id,
                            injected_count = count,
                            "injected system prompts in main loop"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to process chat"
                        );
                    }
                }
            }
        }

        // Wait before next check
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Perform startup reconciliation.
///
/// This checks all monitored chats and injects any missing system prompts.
/// This handles chats that existed before the plugin started.
async fn startup_reconciliation(
    client: &ChatClient,
    chat_monitor: &ChatMonitor,
    cached_prompts: &[CachedPrompt],
) -> Result<usize, PluginError> {
    let mut total_injected = 0;

    for chat_id in chat_monitor.get_chat_ids().await {
        if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
            match system_prompt::process_chat(client, &chat_state, cached_prompts).await {
                Ok(count) => {
                    total_injected += count;
                }
                Err(e) => {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to process chat during startup reconciliation"
                    );
                }
            }
        }
    }

    Ok(total_injected)
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
    #[error("failed to inject system prompt: {0}")]
    Inject(#[from] system_prompt::InjectError),
}
