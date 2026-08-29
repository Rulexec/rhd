//! `customEventAcknowledged` event data.
//!
//! Sent to the plugin that originally sent the custom event when another plugin acknowledges it.

use serde::{Deserialize, Serialize};

/// Data payload for the `customEventAcknowledged` event.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "acknowledgingPluginId": "acknowledging-plugin-id",
///   "isRejected": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventAcknowledgedData {
    /// Unique identifier for the event that was acknowledged.
    pub event_id: String,
    /// ID of the plugin that acknowledged the event.
    pub acknowledging_plugin_id: String,
    /// Whether the event was rejected (false means accepted).
    #[serde(default)]
    pub is_rejected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_event_acknowledged_data_serialization() {
        let data = CustomEventAcknowledgedData {
            event_id: "test-event-id".to_string(),
            acknowledging_plugin_id: "ack-plugin".to_string(),
            is_rejected: true,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        assert!(json.contains("\"acknowledgingPluginId\":\"ack-plugin\""));
        assert!(json.contains("\"isRejected\":true"));

        let deserialized: CustomEventAcknowledgedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_custom_event_acknowledged_data_with_acceptance() {
        let data = CustomEventAcknowledgedData {
            event_id: "test-event-id".to_string(),
            acknowledging_plugin_id: "ack-plugin".to_string(),
            is_rejected: false,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"isRejected\":false"));

        let deserialized: CustomEventAcknowledgedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_custom_event_acknowledged_data_default_rejection() {
        // Test that missing is_rejected field defaults to false
        let json = r#"{"eventId":"test-event-id","acknowledgingPluginId":"ack-plugin"}"#;
        let data: CustomEventAcknowledgedData = serde_json::from_str(json).unwrap();
        assert_eq!(data.event_id, "test-event-id");
        assert_eq!(data.acknowledging_plugin_id, "ack-plugin");
        assert_eq!(data.is_rejected, false);
    }
}
