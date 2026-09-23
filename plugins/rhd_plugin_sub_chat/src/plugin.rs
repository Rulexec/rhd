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

/// Run the sub-chat plugin.
///
/// Lifecycle:
/// 1. Load embedded tool-definition templates
/// 2. Connect to chat server
/// 3. Register as plugin
/// 4. Process pending acks
/// 5. Subscribe to custom events and acknowledge them unhandled (the plugin
///    reacts to none yet — never acking would park senders' coordination
///    waits, e.g. `ai_completions:preRequest` / `ai_completions:preDrainQueue`,
///    which would then stall this plugin's own subchats)
/// 6. Create chat monitor, subscribe to all chats
/// 7. On every chat state change: register the three sub-chat tools once per chat
/// 8. Main loop (keep alive)
///
/// Tool calls are deliberately not handled in this phase — the `on_tool_call`
/// subscription and handlers arrive with the next phases.
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

    // Acknowledge every live custom event. The sub-chat plugin reacts to none
    // of them yet, but senders (e.g. ai_completions on `preRequest` /
    // `preDrainQueue`) wait for an ack from every registered plugin — a
    // missing ack here would stall the coordination timeout, and once
    // subchats exist it would park this plugin's own chats. See
    // plugins/README.md, "Acknowledging Unhandled Events".
    let client_for_events = Arc::clone(&client);
    let _custom_event_token = client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        async move {
            tracing::debug!(
                event_id = %event.event_id,
                event_name = %event.event_name,
                "acknowledging unhandled custom event"
            );
            let _ = client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                })
                .await;
        }
    });

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

    // Chats we already registered the tools for (avoids redundant addTools calls).
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

                let tools: Vec<ToolDefinition> = match parse_tool_definitions(&templates) {
                    Ok(tools) => tools,
                    Err(e) => {
                        tracing::error!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to parse tool definitions"
                        );
                        return;
                    }
                };

                tracing::info!(chat_id = chat_id, "registering sub-chat tools");
                if let Err(e) = client
                    .add_tools(AddToolsParams {
                        chat_id,
                        tools,
                    })
                    .await
                {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to register tools"
                    );
                    return; // retry on the next state change for this chat
                }

                let mut initialized = initialized_chats.write().await;
                initialized.insert(chat_id);
                tracing::info!(chat_id = chat_id, "sub-chat tools registered");
            });
        })
        .await;

    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Parse the three embedded tool-definition templates into protocol types.
fn parse_tool_definitions(templates: &Templates) -> Result<Vec<ToolDefinition>, serde_json::Error> {
    Ok(vec![
        serde_json::from_str(templates.spawn_definition())?,
        serde_json::from_str(templates.status_definition())?,
        serde_json::from_str(templates.await_definition())?,
    ])
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
