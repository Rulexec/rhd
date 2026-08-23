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
///   "role": "user",
///   "content": "Hello",
///   "createdAt": "2026-08-20T18:00:00Z",
///   "reasoningContent": null,
///   "tags": ["important"]
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
    /// Timestamp when the message was created.
    pub created_at: DateTime<Utc>,
    /// Optional reasoning/thinking content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Tags associated with the message.
    #[serde(default)]
    pub tags: Vec<String>,
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

/// A pending custom event that has not been acknowledged.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "eventName": "my-custom-event",
///   "senderPluginId": "sender-plugin",
///   "additional": "{\"key\": \"value\"}",
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
    /// Timestamp when the event was created.
    pub created_at: DateTime<Utc>,
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
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            reasoning_content: None,
            tags: vec!["important".to_string()],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"id\":456"));
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
        // reasoning_content is None, should be skipped
        assert!(!json.contains("reasoningContent"));

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
            created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
            reasoning_content: Some("Thinking...".to_string()),
            tags: vec![],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));

        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
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
}
