//! `subscribePluginsList` method types.
//!
//! Subscribe to changes in the plugins list.

use serde::{Deserialize, Serialize};

/// Parameters for the `subscribePluginsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribePluginsListParams {}

/// Result of the `subscribePluginsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribePluginsListResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_plugins_list_params_serialization() {
        let params = SubscribePluginsListParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: SubscribePluginsListParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_subscribe_plugins_list_result_serialization() {
        let result = SubscribePluginsListResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SubscribePluginsListResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
