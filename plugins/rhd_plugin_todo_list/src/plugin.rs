//! Main plugin logic.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, AddToolsParams, CustomEventData, GetPendingAcksParams,
    RegisterPluginParams, ToolDefinition,
};
use rhd_chat_client::ChatClient;
use tokio::sync::RwLock;

use crate::templates::Templates;
use crate::todo_store::TodoStore;
use crate::tool_handler;

/// Run the todo list plugin.
///
/// This function implements the complete plugin lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create monitors
/// 5. Subscribe to all chats
/// 6. Register callbacks for:
///    - Chat state changes (detect new chats)
///    - Tool calls for rhd_set_todo_list
///    - ai_completions:preRequest custom events
/// 7. Main loop (keep alive)
pub async fn run_plugin(server_url: &str, plugin_id: &str) -> Result<(), PluginError> {
    // Load templates
    let templates = Arc::new(Templates::load().map_err(|e| PluginError::Template(e.to_string()))?);
    tracing::info!("Templates loaded");

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

    // Process pending acks
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

    // Create todo store
    let todo_store = Arc::new(TodoStore::new());

    // Track which chats have been initialized (contract message + tool registered)
    let initialized_chats = Arc::new(RwLock::new(std::collections::HashSet::new()));

    // Register callback for chat state changes (detect new chats)
    let client_for_chat = Arc::clone(&client);
    let templates_for_chat = Arc::clone(&templates);
    let initialized_chats_for_chat = Arc::clone(&initialized_chats);

    chat_monitor
        .on_chat_state_change(move |chat_id, chat_state| {
            let client = Arc::clone(&client_for_chat);
            let templates = Arc::clone(&templates_for_chat);
            let initialized_chats = Arc::clone(&initialized_chats_for_chat);
            let messages = chat_state.messages.clone();

            tokio::spawn(async move {
                // Check if already initialized
                {
                    let initialized = initialized_chats.read().await;
                    if initialized.contains(&chat_id) {
                        return;
                    }
                }

                // Check if contract message already exists
                let has_contract = messages.iter().any(|m| {
                    m.role == "system" && m.tags.iter().any(|t| t == crate::templates::CONTRACT_TAG)
                });

                if has_contract {
                    tracing::debug!(
                        chat_id = chat_id,
                        "contract message already exists, marking as initialized"
                    );
                    let mut initialized = initialized_chats.write().await;
                    initialized.insert(chat_id);
                    return;
                }

                // Add contract system message
                tracing::info!(chat_id = chat_id, "adding contract system message");
                let add_result = client
                    .add_message(AddMessageParams {
                        chat_id,
                        role: "system".to_string(),
                        content: templates.contract().to_string(),
                        tool_call_id: None,
                        reasoning_content: None,
                        tags: vec![crate::templates::CONTRACT_TAG.to_string()],
                        is_finished: true,
                        is_streaming: false,
                    })
                    .await;

                if let Err(e) = add_result {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to add contract message"
                    );
                    return;
                }

                // Register tool
                tracing::info!(chat_id = chat_id, "registering rhd_set_todo_list tool");
                let tool_def: ToolDefinition = match serde_json::from_str(templates.tool_definition())
                {
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

                let add_tools_result = client
                    .add_tools(AddToolsParams {
                        chat_id,
                        tools: vec![tool_def],
                    })
                    .await;

                if let Err(e) = add_tools_result {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to register tool"
                    );
                    return;
                }

                // Mark as initialized
                let mut initialized = initialized_chats.write().await;
                initialized.insert(chat_id);
                tracing::info!(chat_id = chat_id, "chat initialized");
            });
        })
        .await;

    // Subscribe to tool calls for rhd_set_todo_list
    let client_for_tools = Arc::clone(&client);
    let todo_store_for_tools = Arc::clone(&todo_store);
    let templates_for_tools = Arc::clone(&templates);

    client.on_tool_call(
        0, // chat_id 0 means all chats (we'll filter in the handler)
        vec!["rhd_set_todo_list".to_string()],
        move |event| {
            let client = Arc::clone(&client_for_tools);
            let todo_store = Arc::clone(&todo_store_for_tools);
            let templates = Arc::clone(&templates_for_tools);

            async move {
                let chat_id = event.chat_id;
                tracing::debug!(
                    chat_id = chat_id,
                    tool_count = event.message.tool_calls.len(),
                    "received tool call event"
                );

                if let Err(e) =
                    tool_handler::handle_tool_call(client, todo_store, templates, event).await
                {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to handle tool call"
                    );
                }
            }
        },
    );
    tracing::info!("Subscribed to rhd_set_todo_list tool calls");

    // Subscribe to ai_completions:preRequest custom events
    let client_for_events = Arc::clone(&client);
    let chat_monitor_for_events = Arc::clone(&chat_monitor);
    let todo_store_for_events = Arc::clone(&todo_store);
    let templates_for_events = Arc::clone(&templates);

    client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        let _chat_monitor = Arc::clone(&chat_monitor_for_events);
        let todo_store = Arc::clone(&todo_store_for_events);
        let templates = Arc::clone(&templates_for_events);

        async move {
            let event_id = event.event_id.clone();

            // Only handle ai_completions:preRequest events
            if event.event_name != "ai_completions:preRequest" {
                // Acknowledge events we don't handle to avoid blocking the system
                tracing::debug!(
                    event_id = %event_id,
                    event_name = %event.event_name,
                    "acknowledging unhandled event"
                );
                let _ = client
                    .ack_custom_event(AckCustomEventParams {
                        event_id: event_id.clone(),
                        is_rejected: None,
                    })
                    .await;
                return;
            }
            tracing::info!(
                event_id = %event_id,
                "received ai_completions:preRequest event"
            );

            // Extract chat ID from event
            let chat_id = match extract_chat_id_from_event(&event) {
                Some(id) => id,
                None => {
                    tracing::warn!(
                        event_id = %event_id,
                        "event missing chatId, acknowledging without action"
                    );
                    let _ = client
                        .ack_custom_event(AckCustomEventParams {
                            event_id: event_id.clone(),
                            is_rejected: None,
                        })
                        .await;
                    return;
                }
            };

            // Get current todo list
            let todo_items = todo_store.get(chat_id).await;

            // Inject todo list as system message
            let content = match todo_items {
                Some(items) if !items.is_empty() => {
                    tracing::debug!(
                        chat_id = chat_id,
                        items_count = items.len(),
                        "injecting todo list with items"
                    );
                    templates.render_todo_list_with_items(&items)
                }
                _ => {
                    tracing::debug!(chat_id = chat_id, "injecting empty todo list template");
                    templates.todo_list_empty().to_string()
                }
            };

            tracing::warn!(
                event_id = %event_id,
                chat_id = chat_id,
                "about to add todo_list:current system message"
            );

            let add_result = client
                .add_message(AddMessageParams {
                    chat_id,
                    role: "system".to_string(),
                    content,
                    tool_call_id: None,
                    reasoning_content: None,
                    tags: vec!["todo_list:current".to_string()],
                    is_finished: true,
                    is_streaming: false,
                })
                .await;

            tracing::warn!(
                event_id = %event_id,
                chat_id = chat_id,
                "finished adding todo_list:current system message"
            );

            if let Err(e) = add_result {
                tracing::error!(
                    chat_id = chat_id,
                    error = %e,
                    "failed to inject todo list"
                );
            }

            // Acknowledge the event
            match client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event_id.clone(),
                    is_rejected: None,
                })
                .await
            {
                Ok(_) => {
                    tracing::info!(
                        event_id = %event_id,
                        chat_id = chat_id,
                        "acknowledged ai_completions:preRequest event"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        event_id = %event_id,
                        chat_id = chat_id,
                        error = %e,
                        "failed to acknowledge event"
                    );
                }
            }
        }
    });
    tracing::info!("Subscribed to ai_completions:preRequest events");

    // Keep the plugin running
    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Extract chat ID from a custom event.
fn extract_chat_id_from_event(event: &CustomEventData) -> Option<i64> {
    // Try top-level chat_id field first
    if let Some(chat_id) = event
        .chat_id
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
    {
        return Some(chat_id);
    }

    // Fall back to additional JSON
    let additional = event.additional.as_ref()?;
    let json: serde_json::Value = serde_json::from_str(additional).ok()?;
    let chat_id_str = json.get("chatId")?.as_str()?;
    chat_id_str.parse::<i64>().ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_event(chat_id: Option<&str>, additional: Option<&str>) -> CustomEventData {
        CustomEventData {
            event_id: "evt-1".to_string(),
            event_name: "ai_completions:preRequest".to_string(),
            sender_plugin_id: Some("ai_completions".to_string()),
            additional: additional.map(|s| s.to_string()),
            chat_id: chat_id.map(|s| s.to_string()),
            message_id: None,
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_extract_chat_id_from_top_level_field() {
        let event = make_event(Some("42"), Some(r#"{"triggerReason":"queuedMessages"}"#));
        assert_eq!(extract_chat_id_from_event(&event), Some(42));
    }

    #[test]
    fn test_extract_chat_id_from_legacy_additional() {
        let event = make_event(None, Some(r#"{"chatId":"7"}"#));
        assert_eq!(extract_chat_id_from_event(&event), Some(7));
    }

    #[test]
    fn test_extract_chat_id_prefers_top_level_field() {
        let event = make_event(Some("9"), Some(r#"{"chatId":"7"}"#));
        assert_eq!(extract_chat_id_from_event(&event), Some(9));
    }

    #[test]
    fn test_extract_chat_id_missing_everywhere() {
        let event = make_event(None, Some(r#"{"triggerReason":"queuedMessages"}"#));
        assert_eq!(extract_chat_id_from_event(&event), None);
    }

    #[test]
    fn test_extract_chat_id_invalid_top_level_falls_back() {
        let event = make_event(Some("not-a-number"), Some(r#"{"chatId":"7"}"#));
        assert_eq!(extract_chat_id_from_event(&event), Some(7));
    }
}
