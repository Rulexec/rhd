//! `streamChunk` event data.
//!
//! Emitted when new content is pushed to a stream. Sent to all clients
//! subscribed to the chat (via subscribeChat or streamSubscribe).

use serde::{Deserialize, Serialize};

use crate::methods::stream_push::StreamToolCallDelta;

/// Data payload for the `streamChunk` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "type": "contentDelta",
///   "content": "Hello"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunkData {
    /// ID of the chat the stream belongs to.
    pub chat_id: i64,
    /// Type of chunk: "reasoningDelta", "contentDelta", or "toolCallDelta".
    #[serde(rename = "type")]
    pub chunk_type: String,
    /// Content delta (for reasoningDelta and contentDelta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool call deltas (for toolCallDelta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_content_delta() {
        let data = StreamChunkData {
            chat_id: 123,
            chunk_type: "contentDelta".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"type\":\"contentDelta\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(!json.contains("toolCalls"));
    }

    #[test]
    fn test_stream_chunk_tool_call_delta() {
        let data = StreamChunkData {
            chat_id: 123,
            chunk_type: "toolCallDelta".to_string(),
            content: None,
            tool_calls: Some(vec![StreamToolCallDelta {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: "{\"path\":\"/tmp\"}".to_string(),
            }]),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"type\":\"toolCallDelta\""));
        assert!(json.contains("\"toolCalls\""));
        assert!(!json.contains("\"content\""));
    }
}
