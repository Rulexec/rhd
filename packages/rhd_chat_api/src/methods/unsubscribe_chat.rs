//! `unsubscribeChat` method types.
//!
//! Unsubscribe from chat changes.

use serde::{Deserialize, Serialize};

/// Parameters for the `unsubscribeChat` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeChatParams {
    /// ID of the chat to unsubscribe from.
    pub chat_id: i64,
}

/// Result of the `unsubscribeChat` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeChatResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsubscribe_chat_params_serialization() {
        let params = UnsubscribeChatParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: UnsubscribeChatParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_unsubscribe_chat_result_serialization() {
        let result = UnsubscribeChatResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UnsubscribeChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
