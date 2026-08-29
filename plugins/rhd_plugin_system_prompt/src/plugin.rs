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
            // Only handle ai_completions:preRequest events
            if event.event_name != "ai_completions:preRequest" {
                return;
            }

            let event_id = event.event_id.clone();

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
                    let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                        event_id: event_id.clone(),
                        is_rejected: None,
                    }).await;
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
                    let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                        event_id: event_id.clone(),
                        is_rejected: None,
                    }).await;
                    return;
                }
            };

            // Check if chat has unfinished assistant message
            if system_prompt::has_unfinished_assistant_message(&chat_state) {
                tracing::debug!(
                    chat_id = chat_id,
                    "chat has unfinished assistant message, acknowledging without injection"
                );
                let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                    event_id: event_id.clone(),
                    is_rejected: None,
                }).await;
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
            match client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                event_id: event_id.clone(),
                is_rejected: None,
            }).await {
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

/// Extract chat ID from a custom event's additional data.
///
/// The AI completions plugin includes `chatId` in the event's additional JSON field.
fn extract_chat_id_from_event(event: &rhd_chat_api::CustomEventData) -> Option<i64> {
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
