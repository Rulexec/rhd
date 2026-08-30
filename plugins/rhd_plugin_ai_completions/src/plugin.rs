//! AI completions plugin implementation.
//!
//! This module implements the core plugin logic:
//! 1. Connect to chat server and register as plugin
//! 2. Process pending acknowledgments
//! 3. Create monitors for plugins and chats
//! 4. Main loop: detect trigger conditions and handle AI requests

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use rhd_ai_client::AiClient;
use rhd_chat_api::{
    AckCustomEventParams, GetPendingAcksParams, Message, RegisterPluginParams, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, ChatMonitor};

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
        // Acknowledge pending events
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
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

    // Startup reconciliation (D6): a chat left mid-stream by a previous crash must never
    // trigger again — its partial assistant message must never reach the provider.
    tag_crashed_chats(&client, &chat_monitor).await?;

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

    // Track chats currently being processed to prevent duplicate triggers
    let processing_chats = Arc::new(RwLock::new(HashSet::new()));

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
                    // Check if this chat is already being processed
                    let is_processing = processing_chats.read().await.contains(&chat_id);
                    if is_processing {
                        continue;
                    }

                    tracing::info!(
                        chat_id = chat_id,
                        trigger_reason = ?trigger_reason,
                        "triggering AI completion"
                    );

                    // Mark chat as processing
                    processing_chats.write().await.insert(chat_id);

                    // Handle AI request in a separate task to avoid blocking the main loop
                    let client_clone = Arc::clone(&client);
                    let plugins_monitor_clone = Arc::clone(&plugins_monitor);
                    let ai_client_clone = Arc::clone(&ai_client);
                    let config_clone = config.clone();
                    let plugin_id_clone = plugin_id.to_string();
                    let messages_clone = chat_state.messages.clone();
                    let trigger_reason_clone = trigger_reason.clone();
                    let known_version = chat_state.version;
                    let processing_chats_clone = Arc::clone(&processing_chats);
                    
                    tokio::spawn(async move {
                        let result = ai_request::handle_ai_request(
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
                        .await;
                        
                        // Remove chat from processing set when done
                        processing_chats_clone.write().await.remove(&chat_id);
                        
                        if let Err(e) = result {
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

/// Find the first message that proves the plugin crashed mid-stream.
///
/// An assistant message left with `is_streaming == true` or `is_finished == false`
/// can only be the result of a crash between `add_message` and the
/// `stream_finish` / `update_message` pair.
fn find_unfinished_message(messages: &[Message]) -> Option<&Message> {
    messages.iter().find(|m| m.is_streaming || !m.is_finished)
}

/// Once at startup, reconcile chats that may have been left in an inconsistent state.
///
/// For chats with `ai_completions:running` tag:
/// - If chat has unfinished message → add error tag, remove running tag
/// - If chat is eligible for request → remove running tag only
///
/// For chats without running tag but with unfinished message:
/// - Add error tag (existing behavior)
///
/// `has_error_tag` in `trigger_detection` then suppresses all future triggers for
/// those chats. Chats already carrying the error tag are skipped, making this idempotent
/// across restarts.
async fn tag_crashed_chats(
    client: &ChatClient,
    chat_monitor: &ChatMonitor,
) -> Result<(), PluginError> {
    for chat_id in chat_monitor.get_chat_ids().await {
        let Some(state) = chat_monitor.get_chat_state(chat_id).await else {
            continue;
        };

        let has_running_tag = state.tags.iter().any(|tag| tag == "ai_completions:running");
        let has_error_tag = state.tags.iter().any(|tag| tag == "ai_completions:error");

        // Already parked — nothing to do
        if has_error_tag {
            continue;
        }

        // Check for unfinished message
        let unfinished_message = find_unfinished_message(&state.messages);

        if has_running_tag {
            if let Some(offender) = unfinished_message {
                // Chat was left mid-stream → park it
                tracing::warn!(
                    chat_id = chat_id,
                    message_id = offender.id,
                    is_streaming = offender.is_streaming,
                    is_finished = offender.is_finished,
                    "chat has running tag and unfinished message; parking with error tag"
                );
                client
                    .update_chat(UpdateChatParams {
                        chat_id,
                        title: None,
                        add_tags: vec!["ai_completions:error".to_string()],
                        remove_tags: vec!["ai_completions:running".to_string()],
                    })
                    .await
                    .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
            } else {
                // Chat is in good state, just remove running tag
                tracing::info!(
                    chat_id = chat_id,
                    "chat has running tag but is eligible; removing running tag"
                );
                client
                    .update_chat(UpdateChatParams {
                        chat_id,
                        title: None,
                        add_tags: vec![],
                        remove_tags: vec!["ai_completions:running".to_string()],
                    })
                    .await
                    .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
            }
        } else if let Some(offender) = unfinished_message {
            // Existing behavior: park chats with unfinished messages
            tracing::warn!(
                chat_id = chat_id,
                message_id = offender.id,
                is_streaming = offender.is_streaming,
                is_finished = offender.is_finished,
                "unfinished message found at startup; parking chat with error tag"
            );

            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
        }
    }

    Ok(())
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
    #[error("startup reconciliation failed: {0}")]
    StartupReconciliation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn message(id: i64, is_finished: bool, is_streaming: bool) -> Message {
        Message {
            id,
            chat_id: 1,
            role: "assistant".to_string(),
            content: "partial".to_string(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
            is_finished,
            is_streaming,
            tool_calls: vec![],
        }
    }

    #[test]
    fn finds_streaming_message() {
        let messages = vec![message(1, true, false), message(2, false, true)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), Some(2));
    }

    #[test]
    fn finds_not_finished_message() {
        let messages = vec![message(1, false, false)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), Some(1));
    }

    #[test]
    fn all_finished_messages_is_none() {
        let messages = vec![message(1, true, false), message(2, true, false)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), None);
    }

    #[test]
    fn empty_history_is_none() {
        assert_eq!(find_unfinished_message(&[]).map(|m| m.id), None);
    }
}
