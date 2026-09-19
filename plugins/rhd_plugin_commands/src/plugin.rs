//! Core plugin lifecycle for the slash-commands plugin.
//!
//! Connect → register → pending-acks recovery → custom-event subscription →
//! keepalive loop (same shape as `rhd_plugin_system_prompt`). Every custom
//! event received is acknowledged, handled or not: the emitter
//! (`ai_completions`) waits 30 s for acks and parks the chat on timeout, so
//! skipping an ack is the one thing this plugin must never do.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, CustomEventData, GetPendingAcksParams, PendingEvent, RegisterPluginParams,
};
use rhd_chat_client::ChatClient;

use crate::config::CommandRegistry;
use crate::executor;

/// Run the commands plugin.
///
/// Lifecycle:
/// 1. Connect to chat server (with retry)
/// 2. Register as plugin
/// 3. Process pending acks (recover `preDrainQueue` events missed while down)
/// 4. Subscribe to custom events (`executor::process_queue` on preDrainQueue)
/// 5. Keep running (event-driven; no polling)
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    registry: CommandRegistry,
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

    // Recovery: process preDrainQueue events missed while disconnected.
    let pending = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;

    tracing::info!("Found {} pending acks", pending.pending_events.len());
    for event in pending.pending_events {
        recover_pending_event(&client, &registry, event).await;
    }

    // Live handling. NOTE: rhd_chat_client spawns each subscription callback
    // on its own task (see the deadlock rule in memory/development.md), so
    // the requests awaited inside the handler below never block the WS read
    // task.
    let client_for_events = Arc::clone(&client);
    let registry_for_events = Arc::new(registry);
    client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        let registry = Arc::clone(&registry_for_events);
        async move {
            handle_custom_event(&client, &registry, event).await;
        }
    });

    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Recover one pending event: run the executor for `preDrainQueue` (re-parsing
/// a fully-stripped message is a safe no-op), then acknowledge regardless of
/// name or handling outcome.
async fn recover_pending_event(
    client: &ChatClient,
    registry: &CommandRegistry,
    event: PendingEvent,
) {
    if event.event_name == crate::PRE_DRAIN_QUEUE_EVENT {
        if let Some(chat_id) =
            chat_id_from_fields(event.chat_id.as_deref(), event.additional.as_deref())
        {
            if let Err(e) = executor::process_queue(client, registry, chat_id).await {
                tracing::error!(
                    chat_id = chat_id,
                    "pending preDrainQueue processing failed: {}",
                    e
                );
            }
        }
    }
    let _ = client
        .ack_custom_event(AckCustomEventParams {
            event_id: event.event_id,
            is_rejected: None,
        })
        .await;
}

/// Handle one custom event; returns after acknowledging. NEVER skips the ack.
async fn handle_custom_event(
    client: &ChatClient,
    registry: &CommandRegistry,
    event: CustomEventData,
) {
    let event_id = event.event_id.clone();

    if event.event_name != crate::PRE_DRAIN_QUEUE_EVENT {
        tracing::debug!(
            event_id = %event_id,
            event_name = %event.event_name,
            "acknowledging unhandled event"
        );
        let _ = client
            .ack_custom_event(AckCustomEventParams {
                event_id,
                is_rejected: None,
            })
            .await;
        return;
    }

    let Some(chat_id) = extract_chat_id(&event) else {
        tracing::warn!(
            event_id = %event_id,
            "preDrainQueue event missing chatId, acking without action"
        );
        let _ = client
            .ack_custom_event(AckCustomEventParams {
                event_id,
                is_rejected: None,
            })
            .await;
        return;
    };

    tracing::info!(
        chat_id = chat_id,
        event_id = %event_id,
        "processing preDrainQueue"
    );
    if let Err(e) = executor::process_queue(client, registry, chat_id).await {
        // Partial failures must not park the chat: log and acknowledge anyway.
        tracing::error!(chat_id = chat_id, "queue command processing failed: {}", e);
    }
    let _ = client
        .ack_custom_event(AckCustomEventParams {
            event_id,
            is_rejected: None,
        })
        .await;
}

/// Extract the chat id from a custom event.
///
/// The AI completions plugin sends the chat id in the top-level `chat_id`
/// field of the event. The legacy `additional.chatId` JSON path is kept as a
/// fallback for events produced by older senders.
fn extract_chat_id(event: &CustomEventData) -> Option<i64> {
    chat_id_from_fields(event.chat_id.as_deref(), event.additional.as_deref())
}

/// Shared chat-id resolution for [`CustomEventData`] and [`PendingEvent`]
/// (the two shapes carry the same `chatId` fields).
fn chat_id_from_fields(chat_id: Option<&str>, additional: Option<&str>) -> Option<i64> {
    if let Some(chat_id) = chat_id.and_then(|chat_id_str| chat_id_str.parse::<i64>().ok()) {
        return Some(chat_id);
    }

    let additional = additional?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_event(
        event_name: &str,
        chat_id: Option<&str>,
        additional: Option<&str>,
    ) -> CustomEventData {
        CustomEventData {
            event_id: "evt-1".to_string(),
            event_name: event_name.to_string(),
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
        let event = make_event(
            crate::PRE_DRAIN_QUEUE_EVENT,
            Some("42"),
            Some(r#"{"triggerReason":"queuedMessages"}"#),
        );
        assert_eq!(extract_chat_id(&event), Some(42));
    }

    #[test]
    fn test_extract_chat_id_from_legacy_additional() {
        let event = make_event(
            crate::PRE_DRAIN_QUEUE_EVENT,
            None,
            Some(r#"{"chatId":"7"}"#),
        );
        assert_eq!(extract_chat_id(&event), Some(7));
    }

    #[test]
    fn test_extract_chat_id_prefers_top_level_field() {
        let event = make_event(
            crate::PRE_DRAIN_QUEUE_EVENT,
            Some("9"),
            Some(r#"{"chatId":"7"}"#),
        );
        assert_eq!(extract_chat_id(&event), Some(9));
    }

    #[test]
    fn test_extract_chat_id_missing_everywhere() {
        let event = make_event(
            crate::PRE_DRAIN_QUEUE_EVENT,
            None,
            Some(r#"{"triggerReason":"queuedMessages"}"#),
        );
        assert_eq!(extract_chat_id(&event), None);
    }

    #[test]
    fn test_extract_chat_id_invalid_top_level_falls_back() {
        let event = make_event(
            crate::PRE_DRAIN_QUEUE_EVENT,
            Some("not-a-number"),
            Some(r#"{"chatId":"7"}"#),
        );
        assert_eq!(extract_chat_id(&event), Some(7));
    }
}
