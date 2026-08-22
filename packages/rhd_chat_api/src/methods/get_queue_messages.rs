//! `getQueueMessages` method types.
//!
//! Get all queue messages for a chat.

use serde::{Deserialize, Serialize};

use crate::common::Message;

/// Parameters for the `getQueueMessages` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetQueueMessagesParams {
    /// ID of the chat to get queue messages for.
    pub chat_id: i64,
}

/// Result of the `getQueueMessages` method.
///
/// # Example JSON
/// ```json
/// {
///   "messages": [
///     {
///       "id": 456,
///       "chatId": 123,
///       "role": "user",
///       "content": "Queue message",
///       "createdAt": "2026-08-20T18:00:00Z"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetQueueMessagesResult {
    /// List of queue messages.
    pub messages: Vec<Message>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_get_queue_messages_params_serialization() {
        let params = GetQueueMessagesParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: GetQueueMessagesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_queue_messages_result_serialization() {
        let result = GetQueueMessagesResult {
            messages: vec![Message {
                id: 456,
                chat_id: 123,
                role: "user".to_string(),
                content: "Queue message".to_string(),
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"messages\""));
        assert!(json.contains("\"id\":456"));

        let deserialized: GetQueueMessagesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.messages.len(), deserialized.messages.len());
    }
}
