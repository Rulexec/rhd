//! `listChats` method types.
//!
//! Get list of all chats, optionally filtered by tags.

use crate::common::ChatSummary;
use serde::{Deserialize, Serialize};

/// Parameters for the `listChats` method.
///
/// # Example JSON
/// ```json
/// {
///   "tags": ["tag1", "tag2"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListChatsParams {
    /// Optional tags to filter chats by.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Result of the `listChats` method.
///
/// # Example JSON
/// ```json
/// {
///   "chats": [
///     {
///       "id": 123,
///       "title": "Chat Title",
///       "createdAt": "2026-08-20T18:00:00Z",
///       "updatedAt": "2026-08-20T18:30:00Z",
///       "tags": ["tag1", "tag2"]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListChatsResult {
    /// List of chats matching the filter criteria.
    pub chats: Vec<ChatSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_chats_params_serialization() {
        let params = ListChatsParams {
            tags: vec!["tag1".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"tags\""));

        let deserialized: ListChatsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_list_chats_params_default_tags() {
        let json = r#"{}"#;
        let params: ListChatsParams = serde_json::from_str(json).unwrap();
        assert!(params.tags.is_empty());
    }

    #[test]
    fn test_list_chats_result_serialization() {
        use chrono::Utc;

        let result = ListChatsResult {
            chats: vec![ChatSummary {
                id: 123,
                title: "Test".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec![],
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"chats\""));
        assert!(json.contains("\"id\":123"));

        let deserialized: ListChatsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
