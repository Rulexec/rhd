//! `subscribeChatsList` method types.
//!
//! Subscribe to changes in the chats list (chat created, updated, deleted, tag changes).

use serde::{Deserialize, Serialize};

/// Parameters for the `subscribeChatsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeChatsListParams {}

/// Result of the `subscribeChatsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeChatsListResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_chats_list_params_serialization() {
        let params = SubscribeChatsListParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: SubscribeChatsListParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_subscribe_chats_list_result_serialization() {
        let result = SubscribeChatsListResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SubscribeChatsListResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
