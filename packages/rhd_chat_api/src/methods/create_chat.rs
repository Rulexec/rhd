//! `createChat` method types.
//!
//! Create a new chat.

use serde::{Deserialize, Serialize};

/// Parameters for the `createChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "title": "Chat Title",
///   "tags": ["tag1", "tag2"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatParams {
    /// Title of the new chat.
    pub title: String,
    /// Optional tags to associate with the chat.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Result of the `createChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatResult {
    /// ID of the newly created chat.
    pub chat_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_chat_params_serialization() {
        let params = CreateChatParams {
            title: "Test Chat".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"title\":\"Test Chat\""));
        assert!(json.contains("\"tags\""));

        let deserialized: CreateChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_create_chat_params_default_tags() {
        let json = r#"{"title":"Test"}"#;
        let params: CreateChatParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.title, "Test");
        assert!(params.tags.is_empty());
    }

    #[test]
    fn test_create_chat_result_serialization() {
        let result = CreateChatResult { chat_id: 123 };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: CreateChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
