//! `pluginRemoved` event data.
//!
//! Emitted when a plugin is removed from the list.

use serde::{Deserialize, Serialize};

/// Data payload for the `pluginRemoved` event.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRemovedData {
    /// Unique identifier for the plugin that was removed.
    pub plugin_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_removed_data_serialization() {
        let data = PluginRemovedData {
            plugin_id: "my-plugin-id".to_string(),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin-id\""));

        let deserialized: PluginRemovedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
