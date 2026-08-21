//! `registerPlugin` method types.
//!
//! Register current WebSocket connection as a plugin with the specified ID.

use serde::{Deserialize, Serialize};

/// Parameters for the `registerPlugin` method.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin-id"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPluginParams {
    /// Unique identifier for the plugin.
    pub plugin_id: String,
}

/// Result of the `registerPlugin` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPluginResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_plugin_params_serialization() {
        let params = RegisterPluginParams {
            plugin_id: "my-plugin-id".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin-id\""));

        let deserialized: RegisterPluginParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_register_plugin_result_serialization() {
        let result = RegisterPluginResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RegisterPluginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
