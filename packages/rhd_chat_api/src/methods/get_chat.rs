//! `getChat` method types.
//!
//! Get chat info and all messages.

use crate::common::{Chat, Message};
use serde::{Deserialize, Serialize};

/// Parameters for the `getChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetChatParams {
    /// ID of the chat to retrieve.
    pub chat_id: i64,
}

/// Result of the `getChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chat": {
///     "id": 123,
///     "title": "Chat Title",
///     "createdAt": "2026-08-20T18:00:00Z",
///     "updatedAt": "2026-08-20T18:30:00Z",
///     "tags": ["tag1", "tag2"]
///   },
///   "messages": [
///     {
///       "id": 456,
///       "chatId": 123,
///       "role": "user",
///       "content": "Hello",
///       "createdAt": "2026-08-20T18:00:00Z",
///       "reasoningContent": null,
///       "tags": ["important"]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetChatResult {
    /// Full chat information.
    pub chat: Chat,
    /// List of messages in the chat.
    pub messages: Vec<Message>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_chat_params_serialization() {
        let params = GetChatParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: GetChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_chat_result_serialization() {
        use chrono::Utc;

        let result = GetChatResult {
            chat: Chat {
                id: 123,
                title: "Test".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec![],
            },
            messages: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"chat\""));
        assert!(json.contains("\"messages\""));

        let deserialized: GetChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
