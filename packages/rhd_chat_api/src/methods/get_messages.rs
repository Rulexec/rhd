//! `getMessages` method types.
//!
//! Query messages with filters (unresolved tool calls, tags).

use crate::common::Message;
use serde::{Deserialize, Serialize};

/// Parameters for the `getMessages` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "withUnresolvedToolCalls": true,
///   "withAllTags": ["tag1", "tag2"],
///   "withAnyTag": ["tag3"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesParams {
    /// ID of the chat to retrieve messages from.
    pub chat_id: i64,
    /// If true, return only messages with unresolved tool calls.
    #[serde(default)]
    pub with_unresolved_tool_calls: bool,
    /// Return only messages that have ALL of these tags.
    #[serde(default)]
    pub with_all_tags: Vec<String>,
    /// Return only messages that have AT LEAST ONE of these tags.
    #[serde(default)]
    pub with_any_tag: Vec<String>,
}

/// Result of the `getMessages` method.
///
/// # Example JSON
/// ```json
/// {
///   "messages": [
///     {
///       "id": 456,
///       "chatId": 123,
///       "role": "user",
///       "content": "Hello",
///       "createdAt": "2026-08-20T18:00:00Z",
///       "tags": ["important"]
///     }
///   ],
///   "chatVersion": 5
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesResult {
    /// List of messages matching the filters.
    pub messages: Vec<Message>,
    /// Chat version at the time of query. Plugin can compare with cached version
    /// to verify state consistency.
    pub chat_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_messages_params_serialization() {
        let params = GetMessagesParams {
            chat_id: 123,
            with_unresolved_tool_calls: true,
            with_all_tags: vec!["tag1".to_string(), "tag2".to_string()],
            with_any_tag: vec!["tag3".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"withUnresolvedToolCalls\":true"));
        assert!(json.contains("\"withAllTags\":[\"tag1\",\"tag2\"]"));
        assert!(json.contains("\"withAnyTag\":[\"tag3\"]"));

        let deserialized: GetMessagesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_messages_params_defaults() {
        let json = r#"{"chatId":123}"#;
        let params: GetMessagesParams = serde_json::from_str(json).unwrap();

        assert_eq!(params.chat_id, 123);
        assert!(!params.with_unresolved_tool_calls);
        assert!(params.with_all_tags.is_empty());
        assert!(params.with_any_tag.is_empty());
    }

    #[test]
    fn test_get_messages_result_serialization() {
        use chrono::Utc;

        let result = GetMessagesResult {
            messages: vec![Message {
                id: 456,
                chat_id: 123,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec!["important".to_string()],
                is_finished: true,
                is_streaming: false,
                tool_calls: vec![],
            }],
            chat_version: 5,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"messages\""));
        assert!(json.contains("\"chatVersion\":5"));

        let deserialized: GetMessagesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
