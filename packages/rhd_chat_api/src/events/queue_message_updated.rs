//! `queueMessageUpdated` event data.
//!
//! Emitted when a queue message is updated. Sent to all clients subscribed to that chat.

use crate::common::Message;
use serde::{Deserialize, Serialize};

/// Data payload for the `queueMessageUpdated` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "message": {
///     "id": 456,
///     "chatId": 123,
///     "role": "user",
///     "content": "Updated queue message",
///     "createdAt": "2026-08-20T18:00:00Z"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueMessageUpdatedData {
    /// ID of the chat the queue message belongs to.
    pub chat_id: i64,
    /// The updated queue message.
    pub message: Message,
    /// Version of the chat after queue message update.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_queue_message_updated_data_serialization() {
        let data = QueueMessageUpdatedData {
            chat_id: 123,
            message: Message {
                id: 456,
                chat_id: 123,
                role: "user".to_string(),
                content: "Updated queue message".to_string(),
                tool_call_id: None,
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
                tool_calls: vec![],
            },
            chat_version: 3,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"message\""));

        let deserialized: QueueMessageUpdatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
