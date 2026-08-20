//! `updateChat` method types.
//!
//! Partially update a chat. All fields are optional — only provided fields are changed.
//! Tags use additive/subtractive arrays to avoid races with other clients.

use serde::{Deserialize, Serialize};

/// Parameters for the `updateChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "title": "New Title",
///   "addTags": ["new-tag"],
///   "removeTags": ["old-tag"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatParams {
    /// ID of the chat to update.
    pub chat_id: i64,
    /// New title for the chat (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Tags to add to the chat (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_tags: Vec<String>,
    /// Tags to remove from the chat (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_tags: Vec<String>,
}

/// Result of the `updateChat` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_chat_params_serialization() {
        let params = UpdateChatParams {
            chat_id: 123,
            title: Some("New Title".to_string()),
            add_tags: vec!["new-tag".to_string()],
            remove_tags: vec!["old-tag".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"title\":\"New Title\""));
        assert!(json.contains("\"addTags\""));
        assert!(json.contains("\"removeTags\""));

        let deserialized: UpdateChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_update_chat_params_optional_fields() {
        let json = r#"{"chatId":123}"#;
        let params: UpdateChatParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert!(params.title.is_none());
        assert!(params.add_tags.is_empty());
        assert!(params.remove_tags.is_empty());
    }

    #[test]
    fn test_update_chat_result_serialization() {
        let result = UpdateChatResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UpdateChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
