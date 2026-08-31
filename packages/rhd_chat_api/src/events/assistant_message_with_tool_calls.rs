//! `assistantMessageWithToolCalls` event data.
//!
//! Emitted when an assistant message with tool calls is added to a chat.
//! This allows plugins to subscribe to specific tool calls they handle.

use crate::common::Message;
use serde::{Deserialize, Serialize};

/// Data payload for the `assistantMessageWithToolCalls` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "message": {
///     "id": 456,
///     "chatId": 123,
///     "role": "assistant",
///     "content": "",
///     "toolCalls": [
///       {
///         "id": "call_abc123",
///         "type": "function",
///         "function": {
///           "name": "rhd_set_todo_list",
///           "arguments": "{\"todos\": \"[ ] Task 1\"}"
///         }
///       }
///     ],
///     "createdAt": "2026-08-20T18:00:00Z",
///     "isFinished": true,
///     "isStreaming": false
///   },
///   "chatVersion": 2,
///   "toolNames": ["rhd_set_todo_list"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessageWithToolCallsData {
    /// ID of the chat the message was added to.
    pub chat_id: i64,
    /// The assistant message containing tool calls.
    pub message: Message,
    /// Version of the chat after message addition.
    pub chat_version: i64,
    /// Extracted tool names for efficient filtering by subscribers.
    pub tool_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{FunctionCall, ToolCall};
    use chrono::Utc;

    #[test]
    fn test_assistant_message_with_tool_calls_serialization() {
        let data = AssistantMessageWithToolCallsData {
            chat_id: 123,
            message: Message {
                id: 456,
                chat_id: 123,
                role: "assistant".to_string(),
                content: String::new(),
                tool_call_id: None,
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
                tool_calls: vec![ToolCall {
                    id: "call_abc123".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: "rhd_set_todo_list".to_string(),
                        arguments: "{\"todos\": \"[ ] Task 1\"}".to_string(),
                    },
                    tags: vec![],
                }],
            },
            chat_version: 2,
            tool_names: vec!["rhd_set_todo_list".to_string()],
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"toolNames\""));
        assert!(json.contains("rhd_set_todo_list"));

        let deserialized: AssistantMessageWithToolCallsData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
