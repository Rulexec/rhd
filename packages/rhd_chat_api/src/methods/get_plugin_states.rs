//! `getPluginStates` method types.
//!
//! Query live (non-removed) plugin states with optional filters.

use serde::{Deserialize, Serialize};

use crate::common::PluginState;

/// Parameters for the `getPluginStates` method. Both filters optional;
/// `{}` returns all live (non-removed) states of all plugins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPluginStatesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

/// Result of the `getPluginStates` method.
///
/// # Example JSON
/// ```json
/// {
///   "states": [
///     {
///       "pluginId": "mcp",
///       "key": "status",
///       "content": "{\"mcp\":[]}",
///       "format": "json",
///       "schema": "mcpStatus:1",
///       "version": 1,
///       "updatedAt": "2026-09-05 22:41:07"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginStatesResult {
    pub states: Vec<PluginState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::StateFormat;
    use serde_json::json;

    #[test]
    fn test_get_plugin_states_params_empty_is_accepted() {
        let params: GetPluginStatesParams = serde_json::from_value(json!({})).unwrap();
        assert_eq!(params, GetPluginStatesParams::default());
    }

    #[test]
    fn test_get_plugin_states_params_filters_serialization() {
        let params = GetPluginStatesParams {
            plugin_id: Some("mcp".to_string()),
            schema: Some("mcpStatus:1".to_string()),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"schema\":\"mcpStatus:1\""));

        let deserialized: GetPluginStatesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_plugin_states_params_omits_none_filters() {
        let params = GetPluginStatesParams {
            plugin_id: Some("mcp".to_string()),
            schema: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(!json.contains("schema"));

        let deserialized: GetPluginStatesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_plugin_states_result_serialization() {
        let result = GetPluginStatesResult {
            states: vec![PluginState {
                plugin_id: "mcp".to_string(),
                key: "status".to_string(),
                content: "{\"mcp\":[]}".to_string(),
                format: StateFormat::Json,
                schema: "mcpStatus:1".to_string(),
                version: 1,
                updated_at: "2026-09-05 22:41:07".to_string(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"states\""));
        assert!(json.contains("\"pluginId\":\"mcp\""));

        let deserialized: GetPluginStatesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
