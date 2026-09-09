//! Shared types for the chat WebSocket protocol.
//!
//! This module contains the core data types used across the API:
//! - [`Chat`] — Full chat information with metadata
//! - [`Message`] — A message within a chat
//! - [`ChatSummary`] — Lightweight chat info for list views
//! - [`PluginSummary`] — Plugin with active status
//! - [`PendingEvent`] — Pending custom event that has not been acknowledged

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tools::ToolCall;

/// A chat with full metadata.
///
/// # Example JSON
/// ```json
/// {
///   "id": 123,
///   "title": "Chat Title",
///   "createdAt": "2026-08-20T18:00:00Z",
///   "updatedAt": "2026-08-20T18:30:00Z",
///   "tags": ["tag1", "tag2"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    /// Unique identifier for the chat.
    pub id: i64,
    /// Human-readable title of the chat.
    pub title: String,
    /// Timestamp when the chat was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the chat was last updated.
    pub updated_at: DateTime<Utc>,
    /// Tags associated with the chat.
    pub tags: Vec<String>,
    /// Version number for consistency tracking. Increments on any change.
    pub version: i64,
}

/// A message within a chat.
///
/// # Example JSON
/// ```json
/// {
///   "id": 456,
///   "chatId": 123,
///   "role": "tool",
///   "content": "Sunny, 22C",
///   "toolCallId": "call_abc123",
///   "createdAt": "2026-08-20T18:00:00Z",
///   "reasoningContent": null,
///   "tags": ["important"],
///   "isFinished": true,
///   "isStreaming": false
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Unique identifier for the message.
    pub id: i64,
    /// ID of the chat this message belongs to.
    pub chat_id: i64,
    /// Role of the message author (e.g., "user", "assistant").
    pub role: String,
    /// Main content of the message.
    pub content: String,
    /// For `tool`-role messages: the id of the assistant tool call this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Timestamp when the message was created.
    pub created_at: DateTime<Utc>,
    /// Optional reasoning/thinking content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Tags associated with the message.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether the message content is final (no more updates expected).
    #[serde(default = "default_true")]
    pub is_finished: bool,
    /// Whether the message is currently being streamed.
    #[serde(default)]
    pub is_streaming: bool,
    /// Tool calls made by the assistant (empty when the message has none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

fn default_true() -> bool {
    true
}

/// Lightweight chat information for list views.
///
/// Contains the same fields as [`Chat`] but is semantically used
/// when returning chat lists where full message content is not needed.
///
/// # Example JSON
/// ```json
/// {
///   "id": 123,
///   "title": "Chat Title",
///   "createdAt": "2026-08-20T18:00:00Z",
///   "updatedAt": "2026-08-20T18:30:00Z",
///   "tags": ["tag1", "tag2"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    /// Unique identifier for the chat.
    pub id: i64,
    /// Human-readable title of the chat.
    pub title: String,
    /// Timestamp when the chat was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the chat was last updated.
    pub updated_at: DateTime<Utc>,
    /// Tags associated with the chat.
    pub tags: Vec<String>,
    /// Version number for consistency tracking. Increments on any change.
    pub version: i64,
}

/// A plugin with its active status.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id",
///   "isActive": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    /// Unique identifier for the plugin.
    pub plugin_id: String,
    /// Whether the plugin's WebSocket connection is active.
    pub is_active: bool,
}

/// Format of a plugin state's `content`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StateFormat {
    Markdown,
    Json,
}

/// A single named state exposed by a plugin.
///
/// Identified by `(plugin_id, key)`; `version` is server-managed and strictly
/// monotonic per pair across updates and removes (tombstones).
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "mcp",
///   "key": "status",
///   "content": "{\"mcp\":[]}",
///   "format": "json",
///   "schema": "mcpStatus:1",
///   "version": 1,
///   "updatedAt": "2026-09-05 22:41:07"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginState {
    pub plugin_id: String,
    pub key: String,
    pub content: String,
    pub format: StateFormat,
    pub schema: String,
    pub version: i64,
    pub updated_at: String,
}

/// One `(pluginId, key, version)` entry of `subscribePluginStates` params:
/// the version the client already holds. The server returns the current state
/// for every ref whose stored version is greater (catch-up), or nothing newer.
/// `version: 0` means "I hold nothing — send the latest".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateVersionRef {
    pub plugin_id: String,
    pub key: String,
    pub version: i64,
}

/// A pending custom event that has not been acknowledged.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "eventName": "my-custom-event",
///   "senderPluginId": "sender-plugin",
///   "additional": "{\"key\": \"value\"}",
///   "chatId": "chat-123",
///   "messageId": "message-456",
///   "toolCallId": "call-789",
///   "createdAt": "2026-08-20T18:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingEvent {
    /// Unique identifier for the event.
    pub event_id: String,
    /// Name of the event.
    pub event_name: String,
    /// ID of the plugin that sent the event.
    pub sender_plugin_id: Option<String>,
    /// Additional JSON data.
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

/// A tool call accumulated during streaming.
///
/// # Example JSON
/// ```json
/// {
///   "id": "tool_call_1",
///   "name": "search",
///   "arguments": "{\"query\": \"test\"}"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamToolCall {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Name of the tool being called.
    pub name: String,
    /// Accumulated arguments (JSON string).
    pub arguments: String,
}

impl From<Chat> for ChatSummary {
    fn from(chat: Chat) -> Self {
        ChatSummary {
            id: chat.id,
            title: chat.title,
            created_at: chat.created_at,
            updated_at: chat.updated_at,
            tags: chat.tags,
            version: chat.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_serialization() {
        let chat = Chat {
            id: 123,
            title: "Test Chat".to_string(),
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            updated_at: "2026-08-20T18:30:00Z".parse().unwrap(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            version: 1,
        };

        let json = serde_json::to_string(&chat).unwrap();
        assert!(json.contains("\"id\":123"));
        assert!(json.contains("\"title\":\"Test Chat\""));
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        assert!(json.contains("\"tags\""));

        let deserialized: Chat = serde_json::from_str(&json).unwrap();
        assert_eq!(chat, deserialized);
    }

    #[test]
    fn test_message_serialization() {
        let message = Message {
            id: 456,
            chat_id: 123,
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_call_id: None,
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            reasoning_content: None,
            tags: vec!["important".to_string()],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"id\":456"));
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
        // reasoning_content is None, should be skipped
        assert!(!json.contains("reasoningContent"));
        // empty tool_calls should be skipped
        assert!(!json.contains("toolCalls"));

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_message_with_reasoning_content() {
        let message = Message {
            id: 456,
            chat_id: 123,
            role: "assistant".to_string(),
            content: "Answer".to_string(),
            tool_call_id: None,
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            reasoning_content: Some("Thinking...".to_string()),
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_message_tool_call_id_serialization() {
        let message = Message {
            id: 456,
            chat_id: 123,
            role: "tool".to_string(),
            content: "Sunny, 22C".to_string(),
            tool_call_id: Some("call_abc123".to_string()),
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"toolCallId\":\"call_abc123\""));

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_message_omits_tool_call_id_when_none() {
        let json = r#"{"id":1,"chatId":1,"role":"user","content":"Hi","createdAt":"2026-08-20T18:00:00Z","isFinished":true,"isStreaming":false}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(message.tool_call_id, None);

        let out = serde_json::to_string(&message).unwrap();
        assert!(!out.contains("toolCallId"));
    }

    #[test]
    fn test_chat_summary_from_chat() {
        let chat = Chat {
            id: 123,
            title: "Test Chat".to_string(),
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            updated_at: "2026-08-20T18:30:00Z".parse().unwrap(),
            tags: vec!["tag1".to_string()],
            version: 1,
        };

        let summary: ChatSummary = chat.clone().into();
        assert_eq!(summary.id, chat.id);
        assert_eq!(summary.title, chat.title);
        assert_eq!(summary.created_at, chat.created_at);
        assert_eq!(summary.updated_at, chat.updated_at);
        assert_eq!(summary.tags, chat.tags);
    }

    #[test]
    fn test_pending_event_with_context_fields() {
        let event = PendingEvent {
            event_id: "test-event-id".to_string(),
            event_name: "my-event".to_string(),
            sender_plugin_id: Some("sender".to_string()),
            additional: None,
            chat_id: Some("chat-123".to_string()),
            message_id: Some("message-456".to_string()),
            tool_call_id: Some("call-789".to_string()),
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"chatId\":\"chat-123\""));
        assert!(json.contains("\"messageId\":\"message-456\""));
        assert!(json.contains("\"toolCallId\":\"call-789\""));

        let deserialized: PendingEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_pending_event_without_context_fields() {
        let event = PendingEvent {
            event_id: "test-event-id".to_string(),
            event_name: "my-event".to_string(),
            sender_plugin_id: None,
            additional: None,
            chat_id: None,
            message_id: None,
            tool_call_id: None,
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("chatId"));
        assert!(!json.contains("messageId"));
        assert!(!json.contains("toolCallId"));

        let deserialized: PendingEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_plugin_state_serialization() {
        let state = PluginState {
            plugin_id: "mcp".to_string(),
            key: "status".to_string(),
            content: "{\"mcp\":[]}".to_string(),
            format: StateFormat::Json,
            schema: "mcpStatus:1".to_string(),
            version: 3,
            updated_at: "2026-09-05 22:41:07".to_string(),
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"format\":\"json\""));
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("\"updatedAt\":\"2026-09-05 22:41:07\""));

        let deserialized: PluginState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_state_format_serialization() {
        assert_eq!(
            serde_json::to_string(&StateFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&StateFormat::Markdown).unwrap(),
            "\"markdown\""
        );
    }

    #[test]
    fn test_state_format_rejects_unknown_value() {
        assert!(serde_json::from_str::<StateFormat>("\"yaml\"").is_err());
    }
}
