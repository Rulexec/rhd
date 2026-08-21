//! `ackCustomEvent` method types.
//!
//! Acknowledge receiving a custom event by the current plugin.

use serde::{Deserialize, Serialize};

/// Parameters for the `ackCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AckCustomEventParams {
    /// Unique identifier for the event to acknowledge.
    pub event_id: String,
}

/// Result of the `ackCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AckCustomEventResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_custom_event_params_serialization() {
        let params = AckCustomEventParams {
            event_id: "test-event-id".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));

        let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ack_custom_event_result_serialization() {
        let result = AckCustomEventResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: AckCustomEventResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
