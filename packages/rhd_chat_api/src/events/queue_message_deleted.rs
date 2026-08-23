//! `queueMessageDeleted` event data.
//!
//! Emitted when a queue message is deleted. Sent to all clients subscribed to that chat.

use serde::{Deserialize, Serialize};

/// Data payload for the `queueMessageDeleted` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "messageId": 456
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueMessageDeletedData {
    /// ID of the chat the queue message belonged to.
    pub chat_id: i64,
    /// ID of the deleted queue message.
    pub message_id: i64,
    /// Version of the chat after queue message deletion.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_message_deleted_data_serialization() {
        let data = QueueMessageDeletedData {
            chat_id: 123,
            message_id: 456,
            chat_version: 4,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"messageId\":456"));

        let deserialized: QueueMessageDeletedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
