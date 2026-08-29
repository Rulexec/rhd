//! `ackCustomEvent` method types.
//!
//! Acknowledge receiving a custom event by the current plugin.

use serde::{Deserialize, Serialize};

/// Parameters for the `ackCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "isRejected": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AckCustomEventParams {
    /// Unique identifier for the event to acknowledge.
    pub event_id: String,
    /// Whether the event is rejected (defaults to false if not provided).
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rejected: Option<bool>,
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
            is_rejected: Some(true),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        assert!(json.contains("\"isRejected\":true"));

        let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ack_custom_event_params_with_rejection() {
        let params = AckCustomEventParams {
            event_id: "test-event-id".to_string(),
            is_rejected: Some(true),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"isRejected\":true"));

        let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ack_custom_event_params_without_rejection() {
        let params = AckCustomEventParams {
            event_id: "test-event-id".to_string(),
            is_rejected: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        // is_rejected is None, should be skipped
        assert!(!json.contains("isRejected"));

        let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_ack_custom_event_params_default_rejection() {
        // Test that missing is_rejected field defaults to None
        let json = r#"{"eventId":"test-event-id"}"#;
        let params: AckCustomEventParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.event_id, "test-event-id");
        assert_eq!(params.is_rejected, None);
    }

    #[test]
    fn test_ack_custom_event_result_serialization() {
        let result = AckCustomEventResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: AckCustomEventResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
