//! `streamPush` method types.
//!
//! Push streaming content deltas to an active stream.

use serde::{Deserialize, Serialize};

/// A tool call delta during streaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamToolCallDelta {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Tool/function name.
    pub name: String,
    /// Arguments delta (incremental JSON string fragment).
    pub arguments: String,
}

/// Parameters for the `streamPush` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "reasoningContent": "Let me think...",
///   "content": "Hello",
///   "toolCalls": []
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushParams {
    /// ID of the chat whose stream to push to.
    pub chat_id: i64,
    /// Reasoning/thinking content delta (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Main content delta (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool call deltas (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

/// Result of the `streamPush` method.
///
/// # Example JSON
/// ```json
/// {
///   "success": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushResult {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_push_params_serialization() {
        let params = StreamPushParams {
            chat_id: 123,
            reasoning_content: Some("Thinking...".to_string()),
            content: Some("Hello".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(!json.contains("toolCalls"));

        let deserialized: StreamPushParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_push_params_minimal() {
        let json = r#"{"chatId":123}"#;
        let params: StreamPushParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert!(params.reasoning_content.is_none());
        assert!(params.content.is_none());
        assert!(params.tool_calls.is_none());
    }

    #[test]
    fn test_stream_push_result_serialization() {
        let result = StreamPushResult { success: true };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
