//! `pluginStateRemoved` event data.
//!
//! Emitted when a state is removed (tombstoned). `version` is the bumped
//! version so consumers can version-gate stale events.

use serde::{Deserialize, Serialize};

/// Data payload for the `pluginStateRemoved` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateRemovedData {
    pub plugin_id: String,
    pub key: String,
    pub version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_state_removed_data_serialization() {
        let data = PluginStateRemovedData {
            plugin_id: "mcp".to_string(),
            key: "status".to_string(),
            version: 4,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"key\":\"status\""));
        assert!(json.contains("\"version\":4"));

        let deserialized: PluginStateRemovedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
