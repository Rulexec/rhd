//! `chatUpdated` event data.
//!
//! Emitted when a chat is updated (title changed, tags changed). Sent to all clients subscribed to chats list.

use crate::common::ChatSummary;
use serde::{Deserialize, Serialize};

/// Data payload for the `chatUpdated` event.
///
/// # Example JSON
/// ```json
/// {
///   "chat": {
///     "id": 123,
///     "title": "Updated Title",
///     "createdAt": "2026-08-20T18:00:00Z",
///     "updatedAt": "2026-08-20T18:30:00Z",
///     "tags": ["tag1", "tag2"]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatUpdatedData {
    /// The updated chat.
    pub chat: ChatSummary,
    /// Version of the chat after update.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_chat_updated_data_serialization() {
        let data = ChatUpdatedData {
            chat: ChatSummary {
                id: 123,
                title: "Updated Title".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                version: 2,
            },
            chat_version: 2,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chat\""));
        assert!(json.contains("\"id\":123"));
        assert!(json.contains("\"title\":\"Updated Title\""));

        let deserialized: ChatUpdatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
