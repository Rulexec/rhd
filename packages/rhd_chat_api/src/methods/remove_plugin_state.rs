//! `removePluginState` method types.
//!
//! Remove (tombstone) a state owned by the calling plugin. The plugin id is
//! derived from the connection's registry entry — there is no `pluginId` param.

use serde::{Deserialize, Serialize};

/// Parameters for the `removePluginState` method.
///
/// # Example JSON
/// ```json
/// { "key": "status" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemovePluginStateParams {
    pub key: String,
}

/// Result of the `removePluginState` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemovePluginStateResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_plugin_state_params_serialization() {
        let params = RemovePluginStateParams {
            key: "status".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"key\":\"status\""));

        let deserialized: RemovePluginStateParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_remove_plugin_state_result_serialization() {
        let result = RemovePluginStateResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RemovePluginStateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
