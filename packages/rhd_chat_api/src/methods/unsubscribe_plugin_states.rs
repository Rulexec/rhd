//! `unsubscribePluginStates` method types.
//!
//! Unsubscribe from plugin-state changes.

use serde::{Deserialize, Serialize};

/// Parameters for the `unsubscribePluginStates` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribePluginStatesParams {}

/// Result of the `unsubscribePluginStates` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribePluginStatesResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsubscribe_plugin_states_params_serialization() {
        let params = UnsubscribePluginStatesParams {};

        let json = serde_json::to_string(&params).unwrap();
        let deserialized: UnsubscribePluginStatesParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_unsubscribe_plugin_states_result_serialization() {
        let result = UnsubscribePluginStatesResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: UnsubscribePluginStatesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
