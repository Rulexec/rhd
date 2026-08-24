//! `updateMessage` method types.
//!
//! Partially update a message. All fields are optional — only provided fields are changed.
//! Tags use additive/subtractive arrays to avoid races with other clients.

use serde::{Deserialize, Serialize};

/// Parameters for the `updateMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 456,
///   "content": "Updated content",
///   "reasoningContent": "Updated reasoning",
///   "role": "assistant",
///   "addTags": ["new-tag"],
///   "removeTags": ["old-tag"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMessageParams {
    /// ID of the message to update.
    pub message_id: i64,
    /// New content for the message (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// New reasoning/thinking content (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// New role for the message (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Tags to add to the message (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_tags: Vec<String>,
    /// Tags to remove from the message (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_tags: Vec<String>,
    /// Set whether the message is finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_finished: Option<bool>,
    /// Set whether the message is being streamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_streaming: Option<bool>,
    /// Set tool calls (JSON string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,
}

/// Result of the `updateMessage` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMessageResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_message_params_serialization() {
        let params = UpdateMessageParams {
            message_id: 456,
            content: Some("Updated".to_string()),
            reasoning_content: Some("Thinking...".to_string()),
            role: Some("assistant".to_string()),
            add_tags: vec!["new-tag".to_string()],
            remove_tags: vec!["old-tag".to_string()],
            is_finished: Some(true),
            is_streaming: Some(false),
            tool_calls: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"messageId\":456"));
        assert!(json.contains("\"content\":\"Updated\""));
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"addTags\""));
        assert!(json.contains("\"removeTags\""));

        let deserialized: UpdateMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_update_message_params_optional_fields() {
        let json = r#"{"messageId":456}"#;
        let params: UpdateMessageParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.message_id, 456);
        assert!(params.content.is_none());
        assert!(params.reasoning_content.is_none());
        assert!(params.role.is_none());
        assert!(params.add_tags.is_empty());
        assert!(params.remove_tags.is_empty());
        assert!(params.is_finished.is_none());
        assert!(params.is_streaming.is_none());
        assert!(params.tool_calls.is_none());
    }

    #[test]
    fn test_update_message_result_serialization() {
        let result = UpdateMessageResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UpdateMessageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
