//! `pluginStateChanged` event data.
//!
//! Emitted to plugin-state subscribers when a state is created or updated
//! (upsert). Carries the full stored state including the new version.

use serde::{Deserialize, Serialize};

use crate::common::PluginState;

/// Data payload for the `pluginStateChanged` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateChangedData {
    pub state: PluginState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::StateFormat;

    #[test]
    fn test_plugin_state_changed_data_serialization() {
        let data = PluginStateChangedData {
            state: PluginState {
                plugin_id: "mcp".to_string(),
                key: "status".to_string(),
                content: "{\"mcp\":[]}".to_string(),
                format: StateFormat::Json,
                schema: "mcpStatus:1".to_string(),
                version: 2,
                updated_at: "2026-09-05 22:41:07".to_string(),
            },
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"state\""));
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"format\":\"json\""));
        assert!(json.contains("\"version\":2"));

        let deserialized: PluginStateChangedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
