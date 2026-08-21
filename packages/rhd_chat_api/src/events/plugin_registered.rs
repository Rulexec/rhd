//! `pluginRegistered` event data.
//!
//! Emitted when a new plugin is registered or an existing plugin becomes active.

use serde::{Deserialize, Serialize};

/// Data payload for the `pluginRegistered` event.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id",
///   "isActive": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegisteredData {
    /// Unique identifier for the plugin.
    pub plugin_id: String,
    /// Whether the plugin's WebSocket connection is active.
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registered_data_serialization() {
        let data = PluginRegisteredData {
            plugin_id: "my-plugin-id".to_string(),
            is_active: true,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin-id\""));
        assert!(json.contains("\"isActive\":true"));

        let deserialized: PluginRegisteredData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
