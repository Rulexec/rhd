//! `addTools` method types.
//!
//! Add tools to a chat. Tools are associated with the plugin that adds them.

use serde::{Deserialize, Serialize};

use crate::tools::ToolDefinition;

/// Parameters for the `addTools` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "tools": [
///     {
///       "type": "function",
///       "function": {
///         "name": "get_weather",
///         "description": "Get the current weather",
///         "parameters": {}
///       }
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddToolsParams {
    /// ID of the chat to add tools to.
    pub chat_id: i64,
    /// List of tool definitions to add.
    pub tools: Vec<ToolDefinition>,
}

/// Result of the `addTools` method.
///
/// # Example JSON
/// ```json
/// {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddToolsResult {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::FunctionDefinition;

    #[test]
    fn test_add_tools_params_serialization() {
        let params = AddToolsParams {
            chat_id: 123,
            tools: vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: serde_json::json!({}),
                },
            }],
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"get_weather\""));

        let deserialized: AddToolsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_add_tools_result_serialization() {
        let result = AddToolsResult {};

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: AddToolsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
