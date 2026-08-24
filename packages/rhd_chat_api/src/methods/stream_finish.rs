//! `streamFinish` method types.
//!
//! Finish an active stream and update the associated message.

use serde::{Deserialize, Serialize};

use super::stream_push::StreamToolCallDelta;

/// Parameters for the `streamFinish` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "reasoningContent": "Full reasoning text",
///   "content": "Full content text",
///   "toolCalls": []
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishParams {
    /// ID of the chat whose stream to finish.
    pub chat_id: i64,
    /// Final reasoning content (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Final main content (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Final tool calls (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

/// Result of the `streamFinish` method.
///
/// # Example JSON
/// ```json
/// {
///   "success": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishResult {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_finish_params_serialization() {
        let params = StreamFinishParams {
            chat_id: 123,
            reasoning_content: Some("Full reasoning".to_string()),
            content: Some("Full content".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"reasoningContent\":\"Full reasoning\""));
        assert!(json.contains("\"content\":\"Full content\""));

        let deserialized: StreamFinishParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_finish_params_minimal() {
        let json = r#"{"chatId":123}"#;
        let params: StreamFinishParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert!(params.reasoning_content.is_none());
        assert!(params.content.is_none());
        assert!(params.tool_calls.is_none());
    }

    #[test]
    fn test_stream_finish_result_serialization() {
        let result = StreamFinishResult { success: true };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
