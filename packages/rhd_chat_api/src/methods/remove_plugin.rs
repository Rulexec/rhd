//! `removePlugin` method types.
//!
//! Remove a plugin from the list of plugins.

use serde::{Deserialize, Serialize};

/// Parameters for the `removePlugin` method.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemovePluginParams {
    /// Unique identifier for the plugin to remove.
    pub plugin_id: String,
}

/// Result of the `removePlugin` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemovePluginResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_plugin_params_serialization() {
        let params = RemovePluginParams {
            plugin_id: "my-plugin-id".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin-id\""));

        let deserialized: RemovePluginParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_remove_plugin_result_serialization() {
        let result = RemovePluginResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RemovePluginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
