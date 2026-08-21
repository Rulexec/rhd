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
///   "acknowledgingPluginId": "acknowledging-plugin-id"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventAcknowledgedData {
    /// Unique identifier for the event that was acknowledged.
    pub event_id: String,
    /// ID of the plugin that acknowledged the event.
    pub acknowledging_plugin_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_event_acknowledged_data_serialization() {
        let data = CustomEventAcknowledgedData {
            event_id: "test-event-id".to_string(),
            acknowledging_plugin_id: "ack-plugin".to_string(),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"eventId\":\"test-event-id\""));
        assert!(json.contains("\"acknowledgingPluginId\":\"ack-plugin\""));

        let deserialized: CustomEventAcknowledgedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
