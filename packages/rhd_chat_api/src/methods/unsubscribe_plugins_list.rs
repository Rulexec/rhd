//! `unsubscribePluginsList` method types.
//!
//! Unsubscribe from plugins list changes.

use serde::{Deserialize, Serialize};

/// Parameters for the `unsubscribePluginsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribePluginsListParams {}

/// Result of the `unsubscribePluginsList` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribePluginsListResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsubscribe_plugins_list_params_serialization() {
        let params = UnsubscribePluginsListParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: UnsubscribePluginsListParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_unsubscribe_plugins_list_result_serialization() {
        let result = UnsubscribePluginsListResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UnsubscribePluginsListResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
