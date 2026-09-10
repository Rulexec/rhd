//! Main plugin logic.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddToolsParams, GetPendingAcksParams, RegisterPluginParams,
    ToolDefinition,
};
use rhd_chat_client::ChatClient;
use tokio::sync::RwLock;

use crate::templates::Templates;

/// Run the choice plugin.
///
/// Lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create chat monitor, subscribe to all chats
/// 5. On every chat state change: register rhd_choice tool once per chat
/// 6. Main loop (keep alive)
pub async fn run_plugin(server_url: &str, plugin_id: &str) -> Result<(), PluginError> {
    let templates = Arc::new(Templates::load().map_err(|e| PluginError::Template(e.to_string()))?);
    tracing::info!("Templates loaded");

    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    tracing::info!("Connected to chat server");

    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    tracing::info!("Registered as plugin: {}", plugin_id);

    // Drain pending acks so other plugins' coordination is never blocked by us.
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;
    tracing::info!("Subscribed to all chats");

    // Chats we already registered the tool for (avoids redundant addTools calls).
    let initialized_chats = Arc::new(RwLock::new(std::collections::HashSet::new()));

    let client_for_chat = Arc::clone(&client);
    let templates_for_chat = Arc::clone(&templates);
    let initialized_chats_for_chat = Arc::clone(&initialized_chats);

    chat_monitor
        .on_chat_state_change(move |chat_id, _chat_state| {
            let client = Arc::clone(&client_for_chat);
            let templates = Arc::clone(&templates_for_chat);
            let initialized_chats = Arc::clone(&initialized_chats_for_chat);

            // NEVER await inline in the monitor's dispatch path — spawn (see
            // memory/development.md "Event Callbacks Must Not Block").
            tokio::spawn(async move {
                {
                    let initialized = initialized_chats.read().await;
                    if initialized.contains(&chat_id) {
                        return;
                    }
                }

                let tool_def: ToolDefinition =
                    match serde_json::from_str(templates.tool_definition()) {
                        Ok(def) => def,
                        Err(e) => {
                            tracing::error!(
                                chat_id = chat_id,
                                error = %e,
                                "failed to parse tool definition"
                            );
                            return;
                        }
                    };

                tracing::info!(chat_id = chat_id, "registering rhd_choice tool");
                if let Err(e) = client
                    .add_tools(AddToolsParams {
                        chat_id,
                        tools: vec![tool_def],
                    })
                    .await
                {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to register tool"
                    );
                    return; // retry on the next state change for this chat
                }

                let mut initialized = initialized_chats.write().await;
                initialized.insert(chat_id);
                tracing::info!(chat_id = chat_id, "tool registered");
            });
        })
        .await;

    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to load templates: {0}")]
    Template(String),
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
}
