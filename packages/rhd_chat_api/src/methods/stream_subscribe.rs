//! `streamSubscribe` method types.
//!
//! Subscribe to a stream and get current accumulated content.

use serde::{Deserialize, Serialize};

use super::stream_push::StreamToolCallDelta;

/// Parameters for the `streamSubscribe` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeParams {
    /// ID of the chat whose stream to subscribe to.
    pub chat_id: i64,
}

/// Result of the `streamSubscribe` method.
///
/// Returns the current accumulated stream state. The caller is also
/// subscribed to future `streamChunk` events for this chat.
///
/// # Example JSON
/// ```json
/// {
///   "reasoningContent": "Let me think...",
///   "content": "Hello world",
///   "toolCalls": [],
///   "isFinished": false
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeResult {
    /// Accumulated reasoning/thinking content so far.
    pub reasoning_content: String,
    /// Accumulated main content so far.
    pub content: String,
    /// Accumulated tool calls so far.
    pub tool_calls: Vec<StreamToolCallDelta>,
    /// Whether the stream has already finished.
    pub is_finished: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_subscribe_params_serialization() {
        let params = StreamSubscribeParams { chat_id: 123 };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: StreamSubscribeParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_subscribe_result_serialization() {
        let result = StreamSubscribeResult {
            reasoning_content: "Thinking...".to_string(),
            content: "Hello".to_string(),
            tool_calls: vec![],
            is_finished: false,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"isFinished\":false"));

        let deserialized: StreamSubscribeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
