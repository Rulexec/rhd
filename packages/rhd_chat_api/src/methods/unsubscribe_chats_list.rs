//! `unsubscribeChatsList` method types.
//!
//! Unsubscribe from chats list changes.

use serde::{Deserialize, Serialize};

/// Parameters for the `unsubscribeChatsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeChatsListParams {}

/// Result of the `unsubscribeChatsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeChatsListResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsubscribe_chats_list_params_serialization() {
        let params = UnsubscribeChatsListParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: UnsubscribeChatsListParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_unsubscribe_chats_list_result_serialization() {
        let result = UnsubscribeChatsListResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UnsubscribeChatsListResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
