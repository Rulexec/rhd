//! `chatDeleted` event data.
//!
//! Emitted when a chat is deleted. Sent to all clients subscribed to chats list.

use serde::{Deserialize, Serialize};

/// Data payload for the `chatDeleted` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatDeletedData {
    /// ID of the deleted chat.
    pub chat_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_deleted_data_serialization() {
        let data = ChatDeletedData { chat_id: 123 };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: ChatDeletedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
