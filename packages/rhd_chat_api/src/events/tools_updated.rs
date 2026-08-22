//! `toolsUpdated` event data.
//!
//! Emitted when tools are added or removed from a chat. Sent to all clients subscribed to that chat.

use crate::tools::ToolInfo;
use serde::{Deserialize, Serialize};

/// Data payload for the `toolsUpdated` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
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
pub struct ToolsUpdatedData {
    /// ID of the chat whose tools were updated.
    pub chat_id: i64,
    /// The updated list of tools with plugin information.
    pub tools: Vec<ToolInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{FunctionDefinition, ToolDefinition};

    #[test]
    fn test_tools_updated_data_serialization() {
        let data = ToolsUpdatedData {
            chat_id: 123,
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

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"tools\""));
        assert!(json.contains("\"pluginId\":\"my-plugin\""));

        let deserialized: ToolsUpdatedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
