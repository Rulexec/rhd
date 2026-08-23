//! `queueMessageAdded` event data.
//!
//! Emitted when a queue message is added to a chat. Sent to all clients subscribed to that chat.

use crate::common::Message;
use serde::{Deserialize, Serialize};

/// Data payload for the `queueMessageAdded` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "message": {
///     "id": 456,
///     "chatId": 123,
///     "role": "user",
///     "content": "New queue message",
///     "createdAt": "2026-08-20T18:00:00Z"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueMessageAddedData {
    /// ID of the chat the queue message was added to.
    pub chat_id: i64,
    /// The newly added queue message.
    pub message: Message,
    /// Version of the chat after queue message addition.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_queue_message_added_data_serialization() {
        let data = QueueMessageAddedData {
            chat_id: 123,
            message: Message {
                id: 456,
                chat_id: 123,
                role: "user".to_string(),
                content: "New queue message".to_string(),
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
            },
            chat_version: 2,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"id\":456"));

        let deserialized: QueueMessageAddedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
