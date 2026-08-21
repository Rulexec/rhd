//! `getPlugins` method types.
//!
//! List all registered plugins with their active status.

use crate::common::PluginSummary;
use serde::{Deserialize, Serialize};

/// Parameters for the `getPlugins` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginsParams {}

/// Result of the `getPlugins` method.
///
/// # Example JSON
/// ```json
/// {
///   "plugins": [
///     {
///       "pluginId": "my-plugin-id",
///       "isActive": true
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginsResult {
    /// List of all registered plugins.
    pub plugins: Vec<PluginSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_plugins_params_serialization() {
        let params = GetPluginsParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: GetPluginsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_plugins_result_serialization() {
        let result = GetPluginsResult {
            plugins: vec![
                PluginSummary {
                    plugin_id: "plugin-1".to_string(),
                    is_active: true,
                },
                PluginSummary {
                    plugin_id: "plugin-2".to_string(),
                    is_active: false,
                },
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"plugins\""));
        assert!(json.contains("\"pluginId\":\"plugin-1\""));
        assert!(json.contains("\"isActive\":true"));

        let deserialized: GetPluginsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
