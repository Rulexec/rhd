//! Tool definition types for chat.
//!
//! These types are copied from rhd_ai to avoid direct dependency.

use serde::{Deserialize, Serialize};

/// Tool definition for chat (copied from rhd_ai).
///
/// # Example JSON
/// ```json
/// {
///   "type": "function",
///   "function": {
///     "name": "get_weather",
///     "description": "Get the current weather",
///     "parameters": {
///       "type": "object",
///       "properties": {
///         "location": {
///           "type": "string",
///           "description": "The city and state"
///         }
///       },
///       "required": ["location"]
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    /// Tool type (always "function").
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition.
    pub function: FunctionDefinition,
}

/// Function definition within a tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionDefinition {
    /// Name of the function.
    pub name: String,
    /// Description of what the function does.
    pub description: String,
    /// JSON Schema for the function parameters.
    pub parameters: serde_json::Value,
}

/// A tool call made by the assistant.
///
/// # Example JSON
/// ```json
/// {
///   "id": "call_abc123",
///   "type": "function",
///   "function": {
///     "name": "get_weather",
///     "arguments": "{\"location\": \"San Francisco, CA\"}"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Type of tool call (always "function").
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function call details.
    pub function: FunctionCall,
    /// Tags attached to this tool call.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Function call details within a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionCall {
    /// Name of the function being called.
    pub name: String,
    /// JSON string of arguments.
    pub arguments: String,
}

/// Wrapper around ToolDefinition that includes the plugin_id which added this tool.
/// Used in getTools response.
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "my-plugin",
///   "tool": {
///     "type": "function",
///     "function": {
///       "name": "get_weather",
///       "description": "Get the current weather",
///       "parameters": {}
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// ID of the plugin that added this tool.
    pub plugin_id: String,
    /// The tool definition.
    pub tool: ToolDefinition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_serialization() {
        let tool = ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: "Get the current weather".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string"
                        }
                    }
                }),
            },
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"type\":\"function\""));
        assert!(json.contains("\"name\":\"get_weather\""));

        let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tool, deserialized);
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall {
            id: "call_abc123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "get_weather".to_string(),
                arguments: "{\"location\": \"San Francisco\"}".to_string(),
            },
            tags: vec!["reviewed".to_string()],
        };

        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains("\"id\":\"call_abc123\""));
        assert!(json.contains("\"name\":\"get_weather\""));

        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(tool_call, deserialized);
    }

    #[test]
    fn test_tool_info_serialization() {
        let tool_info = ToolInfo {
            plugin_id: "my-plugin".to_string(),
            tool: ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: serde_json::json!({}),
                },
            },
        };

        let json = serde_json::to_string(&tool_info).unwrap();
        assert!(json.contains("\"pluginId\":\"my-plugin\""));
        assert!(json.contains("\"tool\""));

        let deserialized: ToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(tool_info, deserialized);
    }
}
