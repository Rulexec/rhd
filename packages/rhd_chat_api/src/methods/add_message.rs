//! `addMessage` method types.
//!
//! Add a new message to a chat.

use serde::{Deserialize, Serialize};

/// Parameters for the `addMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "role": "tool",
///   "content": "Tool result",
///   "toolCallId": "call_abc123",
///   "reasoningContent": "Thinking...",
///   "tags": ["tag1"]
/// }
/// ```
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddMessageParams {
    /// ID of the chat to add the message to.
    pub chat_id: i64,
    /// Role of the message author (e.g., "user", "assistant").
    pub role: String,
    /// Main content of the message.
    pub content: String,
    /// For `tool`-role messages: the id of the assistant tool call being answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional reasoning/thinking content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Optional tags to associate with the message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Whether the message is finished (default: true).
    #[serde(default = "default_true")]
    pub is_finished: bool,
    /// Whether the message is being streamed (default: false).
    #[serde(default)]
    pub is_streaming: bool,
}

/// Result of the `addMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 789
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddMessageResult {
    /// ID of the newly created message.
    pub message_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_message_params_serialization() {
        let params = AddMessageParams {
            chat_id: 123,
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_call_id: None,
            reasoning_content: Some("Thinking...".to_string()),
            tags: vec!["tag1".to_string()],
            is_finished: true,
            is_streaming: false,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"tags\""));

        let deserialized: AddMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_add_message_params_optional_fields() {
        let json = r#"{"chatId":123,"role":"user","content":"Hello"}"#;
        let params: AddMessageParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert_eq!(params.role, "user");
        assert_eq!(params.content, "Hello");
        assert!(params.tool_call_id.is_none());
        assert!(params.reasoning_content.is_none());
        assert!(params.tags.is_empty());
        assert!(params.is_finished);
        assert!(!params.is_streaming);
    }

    #[test]
    fn test_add_message_params_with_tool_call_id() {
        let json = r#"{"chatId":1,"role":"tool","content":"ok","toolCallId":"call_1"}"#;
        let params: AddMessageParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.tool_call_id, Some("call_1".to_string()));
    }

    #[test]
    fn test_add_message_result_serialization() {
        let result = AddMessageResult { message_id: 789 };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"messageId\":789"));

        let deserialized: AddMessageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
