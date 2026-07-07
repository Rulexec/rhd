use crate::protocol::JsonRpcRequest;
use crate::transport::StdioTransport;
use crate::{McpClientTrait, McpError, McpResult, ToolDefinition, ToolResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct McpClient {
    transport: StdioTransport,
    request_id: AtomicU64,
    tools: Vec<ToolDefinition>,
}

impl McpClient {
    pub async fn kill(&self) -> McpResult<()> {
        self.transport.kill().await
    }

    pub async fn pid(&self) -> Option<u32> {
        self.transport.pid().await
    }
}

impl McpClient {
    pub async fn connect(
        cmd: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &HashMap<String, String>,
    ) -> McpResult<Self> {
        let transport = StdioTransport::spawn(cmd, args, cwd, env).await?;
        let client = Self {
            transport,
            request_id: AtomicU64::new(1),
            tools: Vec::new(),
        };

        client.initialize().await?;
        client.send_initialized_notification().await?;
        let tools = client.list_tools().await?;

        Ok(Self {
            transport: client.transport,
            request_id: client.request_id,
            tools,
        })
    }

    async fn initialize(&self) -> McpResult<()> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "rhd",
                "version": "0.1.0"
            }
        });
        let request = JsonRpcRequest::new(id, "initialize", Some(params));
        let response = self.transport.send_request(&request).await?;

        if response.is_error() {
            return Err(McpError::Protocol(format!(
                "Initialize failed: {:?}",
                response.error
            )));
        }

        Ok(())
    }

    async fn send_initialized_notification(&self) -> McpResult<()> {
        self.transport.send_notification("notifications/initialized", None).await
    }
}

impl McpClientTrait for McpClient {
    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, "tools/list", None);
        let response = self.transport.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(McpError::Protocol(format!(
                "tools/list failed: {}",
                error.message
            )));
        }

        let result = response.result.ok_or_else(|| {
            McpError::Protocol("tools/list returned no result".to_string())
        })?;

        let tools_value = result.get("tools").ok_or_else(|| {
            McpError::Protocol("tools/list result missing 'tools' field".to_string())
        })?;

        let tools: Vec<ToolDefinition> = serde_json::from_value(tools_value.clone())?;
        Ok(tools)
    }

    async fn call_tool(&self, name: &str, arguments: &str) -> McpResult<ToolResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let parsed_arguments: serde_json::Value = serde_json::from_str(arguments)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        let params = serde_json::json!({
            "name": name,
            "arguments": parsed_arguments
        });
        let request = JsonRpcRequest::new(id, "tools/call", Some(params));
        let response = self.transport.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(McpError::Protocol(format!(
                "tools/call failed: {}",
                error.message
            )));
        }

        let result = response.result.ok_or_else(|| {
            McpError::Protocol("tools/call returned no result".to_string())
        })?;

        let content_value = result.get("content").ok_or_else(|| {
            McpError::Protocol("tools/call result missing 'content' field".to_string())
        })?;

        let content_string = match content_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => content_value.to_string(),
        };

        let is_error = result.get("isError").and_then(|v| v.as_bool());

        Ok(ToolResult {
            content: content_string,
            is_error,
        })
    }

    async fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}
