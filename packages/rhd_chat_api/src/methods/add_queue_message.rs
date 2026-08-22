//! `addQueueMessage` method types.
//!
//! Add a new message to the chat's message queue.

use serde::{Deserialize, Serialize};

/// Parameters for the `addQueueMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "role": "user",
///   "content": "New queue message",
///   "reasoningContent": "Thinking...",
///   "tags": ["tag1"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueMessageParams {
    /// ID of the chat to add the queue message to.
    pub chat_id: i64,
    /// Role of the message author (e.g., "user", "assistant").
    pub role: String,
    /// Main content of the message.
    pub content: String,
    /// Optional reasoning/thinking content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Optional tags to associate with the message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Result of the `addQueueMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 789
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueMessageResult {
    /// ID of the newly created queue message.
    pub message_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_queue_message_params_serialization() {
        let params = AddQueueMessageParams {
            chat_id: 123,
            role: "user".to_string(),
            content: "Hello".to_string(),
            reasoning_content: Some("Thinking...".to_string()),
            tags: vec!["tag1".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"tags\""));

        let deserialized: AddQueueMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_add_queue_message_params_optional_fields() {
        let json = r#"{"chatId":123,"role":"user","content":"Hello"}"#;
        let params: AddQueueMessageParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert_eq!(params.role, "user");
        assert_eq!(params.content, "Hello");
        assert!(params.reasoning_content.is_none());
        assert!(params.tags.is_empty());
    }

    #[test]
    fn test_add_queue_message_result_serialization() {
        let result = AddQueueMessageResult { message_id: 789 };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"messageId\":789"));

        let deserialized: AddQueueMessageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
