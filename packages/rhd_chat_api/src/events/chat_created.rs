//! `chatCreated` event data.
//!
//! Emitted when a new chat is created. Sent to all clients subscribed to chats list.

use crate::common::ChatSummary;
use serde::{Deserialize, Serialize};

/// Data payload for the `chatCreated` event.
///
/// # Example JSON
/// ```json
/// {
///   "chat": {
///     "id": 123,
///     "title": "New Chat",
///     "createdAt": "2026-08-20T18:00:00Z",
///     "updatedAt": "2026-08-20T18:00:00Z",
///     "tags": ["tag1"]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatCreatedData {
    /// The newly created chat.
    pub chat: ChatSummary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_chat_created_data_serialization() {
        let data = ChatCreatedData {
            chat: ChatSummary {
                id: 123,
                title: "New Chat".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec!["tag1".to_string()],
            },
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chat\""));
        assert!(json.contains("\"id\":123"));
        assert!(json.contains("\"title\":\"New Chat\""));

        let deserialized: ChatCreatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
