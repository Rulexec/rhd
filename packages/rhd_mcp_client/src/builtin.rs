use crate::{McpClientTrait, McpResult, ToolDefinition, ToolResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct BuiltinTools {
    flags: Arc<Mutex<HashMap<String, bool>>>,
}

impl BuiltinTools {
    pub fn new() -> Self {
        Self {
            flags: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn flags(&self) -> Arc<Mutex<HashMap<String, bool>>> {
        self.flags.clone()
    }

    pub fn list_tool_definitions() -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "rhd_set_flag".to_string(),
            description: "Set a named flag with a boolean value".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Flag name"
                    },
                    "value": {
                        "type": "boolean",
                        "default": true
                    }
                },
                "required": ["name"]
            }),
        }]
    }
}

#[derive(Debug, Deserialize)]
struct SetFlagArgs {
    name: String,
    #[serde(default = "default_true")]
    value: bool,
}

fn default_true() -> bool {
    true
}

impl McpClientTrait for BuiltinTools {
    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>> {
        Ok(Self::list_tool_definitions())
    }

    async fn call_tool(&self, name: &str, arguments: &str) -> McpResult<ToolResult> {
        match name {
            "rhd_set_flag" => {
                let args: SetFlagArgs = serde_json::from_str(arguments).map_err(|e| {
                    crate::McpError::Protocol(format!("Invalid arguments for rhd_set_flag: {}", e))
                })?;

                let mut flags = self.flags.lock().await;
                flags.insert(args.name.clone(), args.value);

                Ok(ToolResult {
                    content: format!("Flag '{}' set to {}", args.name, args.value),
                    is_error: None,
                    raw_response: None,
                })
            }
            _ => Err(crate::McpError::ToolNotFound(name.to_string())),
        }
    }

    async fn has_tool(&self, name: &str) -> bool {
        name == "rhd_set_flag"
    }
}
