//! `subscribePluginStates` method types.
//!
//! Subscribe to plugin-state broadcasts, with versioned catch-up.

use serde::{Deserialize, Serialize};

use crate::common::{PluginState, StateVersionRef};

/// Parameters for the `subscribePluginStates` method.
///
/// `states` is the catch-up list: for each ref the server returns the current
/// state if its version is greater than the passed one. An empty list (or
/// omitted — `#[serde(default)]`) subscribes without catch-up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SubscribePluginStatesParams {
    pub states: Vec<StateVersionRef>,
}

/// Result: current states newer than requested (possibly empty).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribePluginStatesResult {
    pub states: Vec<PluginState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::StateFormat;
    use serde_json::json;

    #[test]
    fn test_subscribe_plugin_states_params_omitted_states_is_accepted() {
        let params: SubscribePluginStatesParams = serde_json::from_value(json!({})).unwrap();
        assert_eq!(params, SubscribePluginStatesParams::default());
    }

    #[test]
    fn test_subscribe_plugin_states_params_serialization() {
        let params = SubscribePluginStatesParams {
            states: vec![StateVersionRef {
                plugin_id: "mcp".to_string(),
                key: "status".to_string(),
                version: 2,
            }],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"states\""));
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"key\":\"status\""));
        assert!(json.contains("\"version\":2"));

        let deserialized: SubscribePluginStatesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_subscribe_plugin_states_result_serialization() {
        let result = SubscribePluginStatesResult {
            states: vec![PluginState {
                plugin_id: "mcp".to_string(),
                key: "status".to_string(),
                content: "{\"mcp\":[]}".to_string(),
                format: StateFormat::Json,
                schema: "mcpStatus:1".to_string(),
                version: 3,
                updated_at: "2026-09-05 22:41:07".to_string(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"states\""));
        assert!(json.contains("\"version\":3"));

        let deserialized: SubscribePluginStatesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
