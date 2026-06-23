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
        let tools = client.list_tools().await?;

        Ok(Self {
            transport: client.transport,
            request_id: client.request_id,
            tools,
        })
    }

    async fn initialize(&self) -> McpResult<()> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, "initialize", None);
        let response = self.transport.send_request(&request).await?;

        if response.is_error() {
            return Err(McpError::Protocol(format!(
                "Initialize failed: {:?}",
                response.error
            )));
        }

        Ok(())
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

        let tools: Vec<ToolDefinition> = serde_json::from_value(result)?;
        Ok(tools)
    }

    async fn call_tool(&self, name: &str, arguments: &str) -> McpResult<ToolResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
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

        let tool_result: ToolResult = serde_json::from_value(result)?;
        Ok(tool_result)
    }

    async fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}
