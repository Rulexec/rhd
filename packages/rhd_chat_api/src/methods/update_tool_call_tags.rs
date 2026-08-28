//! `updateToolCallTags` method types.
//!
//! Add and/or remove tags on a single tool call within a message. The server
//! reads the message's stored tool calls, parses them, applies the tag changes
//! to the tool call with the matching id, saves the result, and broadcasts a
//! `messageUpdated` event so subscribers can sync their state.
//!
//! Tags use additive/subtractive arrays to avoid races with other clients,
//! mirroring `updateMessage`.

use serde::{Deserialize, Serialize};

/// Parameters for the `updateToolCallTags` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 456,
///   "toolCallId": "call_abc123",
///   "addTags": ["reviewed"],
///   "removeTags": ["pending"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolCallTagsParams {
    /// ID of the message containing the tool call.
    pub message_id: i64,
    /// ID of the tool call to tag (matches `toolCalls[].id` within the message).
    pub tool_call_id: String,
    /// Tags to add to the tool call (optional, duplicates are ignored).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_tags: Vec<String>,
    /// Tags to remove from the tool call (optional, missing tags are ignored).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_tags: Vec<String>,
}

/// Result of the `updateToolCallTags` method.
///
/// The new chat version is delivered via the broadcast `messageUpdated` event.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolCallTagsResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_tool_call_tags_params_serialization() {
        let params = UpdateToolCallTagsParams {
            message_id: 456,
            tool_call_id: "call_abc123".to_string(),
            add_tags: vec!["reviewed".to_string()],
            remove_tags: vec!["pending".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"messageId\":456"));
        assert!(json.contains("\"toolCallId\":\"call_abc123\""));
        assert!(json.contains("\"addTags\":[\"reviewed\"]"));
        assert!(json.contains("\"removeTags\":[\"pending\"]"));

        let deserialized: UpdateToolCallTagsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_update_tool_call_tags_params_optional_tag_arrays() {
        let json = r#"{"messageId":456,"toolCallId":"call_1"}"#;
        let params: UpdateToolCallTagsParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.message_id, 456);
        assert_eq!(params.tool_call_id, "call_1");
        assert!(params.add_tags.is_empty());
        assert!(params.remove_tags.is_empty());
    }

    #[test]
    fn test_update_tool_call_tags_result_serialization() {
        let result = UpdateToolCallTagsResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UpdateToolCallTagsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
