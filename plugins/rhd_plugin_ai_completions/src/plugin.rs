//! AI completions plugin implementation.
//!
//! This module implements the core plugin logic:
//! 1. Connect to chat server and register as plugin
//! 2. Process pending acknowledgments
//! 3. Create monitors for plugins and chats
//! 4. Main loop: detect trigger conditions and handle AI requests

use std::sync::Arc;
use std::time::Duration;

use rhd_ai_client::AiClient;
use rhd_chat_api::{AckCustomEventParams, GetPendingAcksParams, RegisterPluginParams};
use rhd_chat_client::ChatClient;

use crate::ai_request;
use crate::config::{self, PluginConfig};
use crate::trigger_detection;

/// Run the AI completions plugin.
///
/// This function implements the complete plugin lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create monitors
/// 5. Subscribe to all chats
/// 6. Create AI client
/// 7. Main loop: check triggers and handle AI requests
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    config: PluginConfig,
) -> Result<(), PluginError> {
    // Connect to chat server
    let client = Arc::new(
        ChatClient::connect(server_url)
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
        // Acknowledge pending events
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: event.event_id,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    // Create plugins monitor
    let plugins_monitor = Arc::new(
        client
            .create_plugins_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );

    tracing::info!("Created plugins monitor");

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

    // Create AI client
    let default_model = &config.ai_completions.models["default"];
    let default_name = "default".to_string();
    let model_name = default_model.alias.as_ref().unwrap_or(&default_name);
    let model_config = &config.ai_completions.models[model_name];

    let api_key = config::resolve_api_key(&config, &model_config.api_key.as_ref().unwrap().cred)
        .map_err(|e| PluginError::Config(e.to_string()))?;

    let ai_client = Arc::new(AiClient::new(
        model_config
            .base_url
            .as_ref()
            .unwrap_or(&String::new())
            .clone(),
        api_key,
    ));

    tracing::info!("Created AI client");

    // Main loop: check for trigger conditions and handle AI requests
    loop {
        // Get all chat IDs
        let chat_ids = chat_monitor.get_chat_ids().await;
        tracing::debug!(
            monitored_chats = chat_ids.len(),
            "main loop iteration"
        );

        for chat_id in chat_ids {
            // Get chat state
            if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
                tracing::debug!(
                    chat_id = chat_id,
                    queued_messages_count = chat_state.queued_messages_count,
                    messages_count = chat_state.messages.len(),
                    tags = ?chat_state.tags,
                    "evaluating chat state"
                );
                // Check trigger condition
                let trigger_reason = trigger_detection::should_trigger(&chat_state);

                if trigger_reason != trigger_detection::TriggerReason::None {
                    tracing::info!(
                        chat_id = chat_id,
                        trigger_reason = ?trigger_reason,
                        "triggering AI completion"
                    );

                    // Handle AI request in a separate task to avoid blocking the main loop
                    let client_clone = Arc::clone(&client);
                    let plugins_monitor_clone = Arc::clone(&plugins_monitor);
                    let ai_client_clone = Arc::clone(&ai_client);
                    let config_clone = config.clone();
                    let plugin_id_clone = plugin_id.to_string();
                    let messages_clone = chat_state.messages.clone();
                    let trigger_reason_clone = trigger_reason.clone();
                    let known_version = chat_state.version;
                    
                    tokio::spawn(async move {
                        if let Err(e) = ai_request::handle_ai_request(
                            client_clone,
                            plugins_monitor_clone,
                            ai_client_clone,
                            &config_clone,
                            &plugin_id_clone,
                            chat_id,
                            &messages_clone,
                            trigger_reason_clone,
                            Some(known_version),
                        )
                        .await
                        {
                            tracing::error!("AI request failed for chat {}: {}", chat_id, e);
                        }
                    });
                }
            }
        }

        // Wait before next check
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
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
    #[error("configuration error: {0}")]
    Config(String),
}
