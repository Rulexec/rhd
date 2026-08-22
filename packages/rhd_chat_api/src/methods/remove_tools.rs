//! `removeTools` method types.
//!
//! Remove tools from a chat by name. Only tools owned by the calling plugin can be removed.

use serde::{Deserialize, Serialize};

/// Parameters for the `removeTools` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "toolNames": ["get_weather", "get_time"]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveToolsParams {
    /// ID of the chat to remove tools from.
    pub chat_id: i64,
    /// List of tool names to remove.
    pub tool_names: Vec<String>,
}

/// Result of the `removeTools` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveToolsResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_tools_params_serialization() {
        let params = RemoveToolsParams {
            chat_id: 123,
            tool_names: vec!["get_weather".to_string(), "get_time".to_string()],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"toolNames\""));
        assert!(json.contains("\"get_weather\""));

        let deserialized: RemoveToolsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_remove_tools_result_serialization() {
        let result = RemoveToolsResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RemoveToolsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
