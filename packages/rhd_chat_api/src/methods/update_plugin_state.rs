//! `updatePluginState` method types.
//!
//! Upsert a state owned by the calling plugin. The plugin id is derived from
//! the connection's registry entry — there is no `pluginId` param.

use serde::{Deserialize, Serialize};

use crate::common::{PluginState, StateFormat};

/// Parameters for the `updatePluginState` method.
///
/// # Example JSON
/// ```json
/// { "key": "status", "content": "{\"mcp\":[]}", "format": "json", "schema": "mcpStatus:1" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginStateParams {
    pub key: String,
    pub content: String,
    pub format: StateFormat,
    pub schema: String,
}

/// Result of the `updatePluginState` method — the stored state including the
/// server-assigned `version` so the writer learns its new version immediately.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginStateResult {
    pub state: PluginState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_plugin_state_params_serialization() {
        let params = UpdatePluginStateParams {
            key: "status".to_string(),
            content: "{\"mcp\":[]}".to_string(),
            format: StateFormat::Json,
            schema: "mcpStatus:1".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"key\":\"status\""));
        assert!(json.contains("\"content\""));
        assert!(json.contains("\"format\":\"json\""));
        assert!(json.contains("\"schema\":\"mcpStatus:1\""));

        let deserialized: UpdatePluginStateParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_update_plugin_state_result_serialization() {
        let result = UpdatePluginStateResult {
            state: PluginState {
                plugin_id: "mcp".to_string(),
                key: "status".to_string(),
                content: "{\"mcp\":[]}".to_string(),
                format: StateFormat::Json,
                schema: "mcpStatus:1".to_string(),
                version: 1,
                updated_at: "2026-09-05 22:41:07".to_string(),
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"state\""));
        assert!(json.contains("\"pluginId\":\"mcp\""));
        assert!(json.contains("\"version\":1"));

        let deserialized: UpdatePluginStateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
