//! `deleteMessage` method types.
//!
//! Delete a message.

use serde::{Deserialize, Serialize};

/// Parameters for the `deleteMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 456
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMessageParams {
    /// ID of the message to delete.
    pub message_id: i64,
}

/// Result of the `deleteMessage` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMessageResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_message_params_serialization() {
        let params = DeleteMessageParams { message_id: 456 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"messageId\":456"));

        let deserialized: DeleteMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_delete_message_result_serialization() {
        let result = DeleteMessageResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DeleteMessageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
