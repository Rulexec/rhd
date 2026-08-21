//! `sendCustomEvent` method types.
//!
//! Send a custom event that broadcasts to all connected WebSockets.

use serde::{Deserialize, Serialize};

/// Parameters for the `sendCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventName": "my-custom-event",
///   "additional": "{\"key\": \"value\"}"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendCustomEventParams {
    /// Name of the custom event.
    pub event_name: String,
    /// Optional additional JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<String>,
}

/// Result of the `sendCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendCustomEventResult {
    /// Unique identifier for the generated event.
    pub event_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_custom_event_params_serialization() {
        let params = SendCustomEventParams {
            event_name: "my-custom-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"eventName\":\"my-custom-event\""));
        assert!(json.contains("\"additional\""));

        let deserialized: SendCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_send_custom_event_params_without_additional() {
        let params = SendCustomEventParams {
            event_name: "my-custom-event".to_string(),
            additional: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"eventName\":\"my-custom-event\""));
        // additional is None, should be skipped
        assert!(!json.contains("additional"));

        let deserialized: SendCustomEventParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_send_custom_event_result_serialization() {
        let result = SendCustomEventResult {
            event_id: "test-event-id".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));

        let deserialized: SendCustomEventResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
