//! `subscribeChat` method types.
//!
//! Subscribe to changes in a specific chat (message additions, edits, deletions, tag changes).

use serde::{Deserialize, Serialize};

/// Parameters for the `subscribeChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeChatParams {
    /// ID of the chat to subscribe to.
    pub chat_id: i64,
}

/// Result of the `subscribeChat` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeChatResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_chat_params_serialization() {
        let params = SubscribeChatParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: SubscribeChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_subscribe_chat_result_serialization() {
        let result = SubscribeChatResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SubscribeChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
