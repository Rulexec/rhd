//! `messageUpdated` event data.
//!
//! Emitted when a message is edited. Sent to all clients subscribed to that chat.

use crate::common::Message;
use serde::{Deserialize, Serialize};

/// Data payload for the `messageUpdated` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "message": {
///     "id": 456,
///     "chatId": 123,
///     "role": "user",
///     "content": "Updated content",
///     "createdAt": "2026-08-20T18:00:00Z",
///     "reasoningContent": "Updated reasoning",
///     "tags": ["tag1"]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageUpdatedData {
    /// ID of the chat the message belongs to.
    pub chat_id: i64,
    /// The updated message.
    pub message: Message,
    /// Version of the chat after message update.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_message_updated_data_serialization() {
        let data = MessageUpdatedData {
            chat_id: 123,
            message: Message {
                id: 456,
                chat_id: 123,
                role: "user".to_string(),
                content: "Updated content".to_string(),
                created_at: Utc::now(),
                reasoning_content: Some("Updated reasoning".to_string()),
                tags: vec!["tag1".to_string()],
            },
            chat_version: 3,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"id\":456"));

        let deserialized: MessageUpdatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
