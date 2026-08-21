//! `pluginUpdated` event data.
//!
//! Emitted when a plugin's active status changes.

use serde::{Deserialize, Serialize};

/// Data payload for the `pluginUpdated` event.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id",
///   "isActive": false
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdatedData {
    /// Unique identifier for the plugin.
    pub plugin_id: String,
    /// Whether the plugin's WebSocket connection is active.
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_updated_data_serialization() {
        let data = PluginUpdatedData {
            plugin_id: "my-plugin-id".to_string(),
            is_active: false,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin-id\""));
        assert!(json.contains("\"isActive\":false"));

        let deserialized: PluginUpdatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
