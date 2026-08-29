//! `customEvent` event data.
//!
//! Broadcast to all connected WebSocket clients when a custom event is sent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data payload for the `customEvent` event.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "eventName": "my-custom-event",
///   "senderPluginId": "sender-plugin-id",
///   "additional": "{\"key\": \"value\"}",
///   "chatId": "chat-123",
///   "messageId": "message-456",
///   "toolCallId": "call-789",
///   "createdAt": "2026-08-20T18:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventData {
    /// Unique identifier for the event.
    pub event_id: String,
    /// Name of the custom event.
    pub event_name: String,
    /// ID of the plugin that sent the event (if sent by a plugin).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_plugin_id: Option<String>,
    /// Optional additional JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<String>,
    /// Optional chat ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Optional message ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Optional tool call ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Timestamp when the event was created.
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_event_data_serialization() {
        let data = CustomEventData {
            event_id: "test-event-id".to_string(),
            event_name: "my-custom-event".to_string(),
            sender_plugin_id: Some("sender-plugin".to_string()),
            additional: Some("{\"key\": \"value\"}".to_string()),
            chat_id: Some("chat-123".to_string()),
            message_id: Some("message-456".to_string()),
            tool_call_id: Some("call-789".to_string()),
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        assert!(json.contains("\"eventName\":\"my-custom-event\""));
        assert!(json.contains("\"senderPluginId\":\"sender-plugin\""));
        assert!(json.contains("\"additional\""));
        assert!(json.contains("\"chatId\":\"chat-123\""));
        assert!(json.contains("\"messageId\":\"message-456\""));
        assert!(json.contains("\"toolCallId\":\"call-789\""));
        assert!(json.contains("\"createdAt\""));

        let deserialized: CustomEventData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_custom_event_data_without_optional_fields() {
        let data = CustomEventData {
            event_id: "test-event-id".to_string(),
            event_name: "my-custom-event".to_string(),
            sender_plugin_id: None,
            additional: None,
            chat_id: None,
            message_id: None,
            tool_call_id: None,
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        // Optional fields should be skipped
        assert!(!json.contains("senderPluginId"));
        assert!(!json.contains("additional"));
        assert!(!json.contains("chatId"));
        assert!(!json.contains("messageId"));
        assert!(!json.contains("toolCallId"));

        let deserialized: CustomEventData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
