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

    // Subscribe to custom events for coordination with AI completions plugin
    let client_for_events = Arc::clone(&client);
    let chat_monitor_for_events = Arc::clone(&chat_monitor);
    let cached_prompts_for_events = cached_prompts.clone();
    let plugin_id_for_events = plugin_id.to_string();

    client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        let chat_monitor = Arc::clone(&chat_monitor_for_events);
        let cached_prompts = cached_prompts_for_events.clone();
        let _plugin_id = plugin_id_for_events.clone();

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
                    .ack_custom_event(rhd_chat_api::AckCustomEventParams {
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
                    // Still acknowledge to avoid blocking
                    let _ = client
                        .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                            event_id: event_id.clone(),
                            is_rejected: None,
                        })
                        .await;
                    return;
                }
            };

            // Get chat state
            let chat_state = match chat_monitor.get_chat_state(chat_id).await {
                Some(state) => state,
                None => {
                    tracing::warn!(
                        chat_id = chat_id,
                        "chat not found in monitor, acknowledging without action"
                    );
                    let _ = client
                        .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                            event_id: event_id.clone(),
                            is_rejected: None,
                        })
                        .await;
                    return;
                }
            };

            // Do not inject while the chat is locked by the AI completions
            // plugin. The event is still acknowledged so the request is not
            // blocked: a running tag here means a tool-loop continuation, and
            // the prompt will be injected once the chat goes idle.
            if system_prompt::has_blocking_tag(&chat_state) {
                tracing::debug!(
                    chat_id = chat_id,
                    "chat has ai_completions running/error tag, acknowledging without injection"
                );
                let _ = client
                    .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                        event_id: event_id.clone(),
                        is_rejected: None,
                    })
                    .await;
                return;
            }

            // Check if chat has unfinished assistant message
            if system_prompt::has_unfinished_assistant_message(&chat_state) {
                tracing::debug!(
                    chat_id = chat_id,
                    "chat has unfinished assistant message, acknowledging without injection"
                );
                let _ = client
                    .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                        event_id: event_id.clone(),
                        is_rejected: None,
                    })
                    .await;
                return;
            }

            // Check if we need to inject system prompts
            let required_names = system_prompt::get_required_prompt_names(&chat_state);
            let mut needs_injection = false;

            for prompt_name in &required_names {
                if !system_prompt::has_system_prompt_message(&chat_state, prompt_name) {
                    needs_injection = true;
                    break;
                }
            }

            if needs_injection {
                // Inject system prompts BEFORE acknowledging
                tracing::info!(
                    chat_id = chat_id,
                    "injecting system prompts before acknowledging event"
                );

                match system_prompt::process_chat(&client, &chat_state, &cached_prompts).await {
                    Ok(count) => {
                        tracing::info!(
                            chat_id = chat_id,
                            injected_count = count,
                            "injected system prompts before ack"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to inject system prompts, acknowledging anyway"
                        );
                    }
                }
            }

            // Acknowledge the event
            match client
                .ack_custom_event(rhd_chat_api::AckCustomEventParams {
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

    tracing::info!("Subscribed to custom events for coordination");

    // Startup reconciliation
    let injected_count = startup_reconciliation(&client, &chat_monitor, &cached_prompts).await?;
    tracing::info!(
        "Startup reconciliation: injected {} system prompts",
        injected_count
    );

    // Register callback for chat state changes
    let client_for_callback = Arc::clone(&client);
    let cached_prompts_for_callback = cached_prompts.clone();

    chat_monitor.on_chat_state_change(move |chat_id, chat_state| {
        let client = Arc::clone(&client_for_callback);
        let cached_prompts = cached_prompts_for_callback.clone();
        let chat_state_clone = chat_state.clone();

        tokio::spawn(async move {
            match system_prompt::process_chat(&client, &chat_state_clone, &cached_prompts).await {
                Ok(count) if count > 0 => {
                    tracing::info!(
                        chat_id = chat_id,
                        injected_count = count,
                        "injected system prompts after state change"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to process chat after state change"
                    );
                }
            }
        });
    }).await;

    // Keep the plugin running (no more polling loop)
    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
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

/// Extract chat ID from a custom event.
///
/// The AI completions plugin sends the chat id in the top-level `chat_id`
/// field of the event. The legacy `additional.chatId` JSON path is kept as a
/// fallback for events produced by older senders.
fn extract_chat_id_from_event(event: &rhd_chat_api::CustomEventData) -> Option<i64> {
    if let Some(chat_id) = event
        .chat_id
        .as_deref()
        .and_then(|chat_id_str| chat_id_str.parse::<i64>().ok())
    {
        return Some(chat_id);
    }

    let additional = event.additional.as_ref()?;
    let json: serde_json::Value = serde_json::from_str(additional).ok()?;
    let chat_id_str = json.get("chatId")?.as_str()?;
    chat_id_str.parse::<i64>().ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::CustomEventData;

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
