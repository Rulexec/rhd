//! `getTools` method types.
//!
//! Get all tools for a chat, including which plugin added each tool.

use serde::{Deserialize, Serialize};

use crate::tools::ToolInfo;

/// Parameters for the `getTools` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetToolsParams {
    /// ID of the chat to get tools for.
    pub chat_id: i64,
}

/// Result of the `getTools` method.
///
/// # Example JSON
/// ```json
/// {
///   "tools": [
///     {
///       "pluginId": "my-plugin",
///       "tool": {
///         "type": "function",
///         "function": {
///           "name": "get_weather",
///           "description": "Get weather",
///           "parameters": {}
///         }
///       }
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetToolsResult {
    /// List of tools with their plugin information.
    pub tools: Vec<ToolInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{FunctionDefinition, ToolDefinition};

    #[test]
    fn test_get_tools_params_serialization() {
        let params = GetToolsParams { chat_id: 123 };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: GetToolsParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_get_tools_result_serialization() {
        let result = GetToolsResult {
            tools: vec![ToolInfo {
                plugin_id: "my-plugin".to_string(),
                tool: ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "get_weather".to_string(),
                        description: "Get weather".to_string(),
                        parameters: serde_json::json!({}),
                    },
                },
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"pluginId\":\"my-plugin\""));

        let deserialized: GetToolsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
