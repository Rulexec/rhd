//! `deleteQueueMessage` method types.
//!
//! Delete a queue message.

use serde::{Deserialize, Serialize};

/// Parameters for the `deleteQueueMessage` method.
///
/// # Example JSON
/// ```json
/// {
///   "messageId": 456
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQueueMessageParams {
    /// ID of the queue message to delete.
    pub message_id: i64,
}

/// Result of the `deleteQueueMessage` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQueueMessageResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_queue_message_params_serialization() {
        let params = DeleteQueueMessageParams { message_id: 456 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"messageId\":456"));

        let deserialized: DeleteQueueMessageParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_delete_queue_message_result_serialization() {
        let result = DeleteQueueMessageResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DeleteQueueMessageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
