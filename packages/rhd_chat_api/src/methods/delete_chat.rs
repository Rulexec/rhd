//! `deleteChat` method types.
//!
//! Delete a chat and all its messages.

use serde::{Deserialize, Serialize};

/// Parameters for the `deleteChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteChatParams {
    /// ID of the chat to delete.
    pub chat_id: i64,
}

/// Result of the `deleteChat` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteChatResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_chat_params_serialization() {
        let params = DeleteChatParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: DeleteChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_delete_chat_result_serialization() {
        let result = DeleteChatResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DeleteChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
