//! `getPendingAcks` method types.
//!
//! Get list of events that have not been acknowledged by the current plugin.

use crate::common::PendingEvent;
use serde::{Deserialize, Serialize};

/// Parameters for the `getPendingAcks` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPendingAcksParams {}

/// Result of the `getPendingAcks` method.
///
/// # Example JSON
/// ```json
/// {
///   "pendingEvents": [
///     {
///       "eventId": "generated-uuid-string",
///       "eventName": "my-custom-event",
///       "senderPluginId": "sender-plugin",
///       "additional": "{\"key\": \"value\"}",
///       "createdAt": "2026-08-20T18:00:00Z"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPendingAcksResult {
    /// List of events that have not been acknowledged.
    pub pending_events: Vec<PendingEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_get_pending_acks_params_serialization() {
        let params = GetPendingAcksParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: GetPendingAcksParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_pending_acks_result_serialization() {
        let result = GetPendingAcksResult {
            pending_events: vec![PendingEvent {
                event_id: "test-event-id".to_string(),
                event_name: "test-event".to_string(),
                sender_plugin_id: Some("sender-plugin".to_string()),
                additional: Some("{\"key\": \"value\"}".to_string()),
                chat_id: None,
                message_id: None,
                tool_call_id: None,
                created_at: Utc::now(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"pendingEvents\""));
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        assert!(json.contains("\"eventName\":\"test-event\""));

        let deserialized: GetPendingAcksResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
