//! MCP server pool: spawns configured servers, owns their clients, and
//! routes prefixed tool names to the owning server.

use std::collections::HashMap;

use rhd_chat_api::{FunctionDefinition, ToolDefinition as ChatToolDefinition};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::{McpClientTrait, ToolDefinition as McpToolDefinition, ToolResult};
use tokio::sync::Mutex;

use crate::config::{PluginConfig, ResolvedServer};

/// Routing target for a prefixed chat tool name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub server_id: String,
    pub tool_name: String,
}

/// One live MCP server with its chat-facing (prefixed) tool definitions.
struct PoolServer {
    config: ResolvedServer,
    client: Mutex<McpClient>,
    chat_tools: Vec<ChatToolDefinition>,
}

/// All configured MCP servers, spawned and ready.
///
/// Dropping the pool kills the child processes (`StdioTransport::Drop`
/// calls `start_kill`), so shutdown is simply letting the `Arc<McpPool>` go.
pub struct McpPool {
    servers: HashMap<String, PoolServer>, // key: server id
    routes: HashMap<String, Route>,       // key: prefixed tool name
}

/// Split `{name}:{tool}` at the first colon. Returns `None` when the name
/// has no prefix or an empty prefix/tool part.
pub fn split_prefixed(prefixed: &str) -> Option<(&str, &str)> {
    let (prefix, rest) = prefixed.split_once(':')?;
    if prefix.is_empty() || rest.is_empty() {
        return None;
    }
    Some((prefix, rest))
}

/// Map an MCP tool definition to the chat-facing shape, prefixing the name
/// with the server's `name` (AD-1). MCP `inputSchema` becomes `parameters`.
pub fn to_chat_tool(server_name: &str, mcp_tool: &McpToolDefinition) -> ChatToolDefinition {
    ChatToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: format!("{}:{}", server_name, mcp_tool.name),
            description: mcp_tool.description.clone(),
            parameters: mcp_tool.input_schema.clone(),
        },
    }
}

impl McpPool {
    /// Spawn and initialize every configured server (fail-fast, AD-4).
    pub async fn startup(config: &PluginConfig) -> Result<Self, PoolError> {
        let mut servers = HashMap::new();
        let mut routes = HashMap::new();

        for entry in &config.servers {
            let client = McpClient::connect(
                &entry.cmd,
                &entry.args,
                entry.cwd.as_deref(), // None → child inherits plugin cwd
                &entry.env,
            )
            .await
            .map_err(|e| PoolError::Spawn {
                server: entry.name.clone(),
                details: e.to_string(),
            })?;

            let mcp_tools = client
                .list_tools()
                .await
                .map_err(|e| PoolError::ListTools {
                    server: entry.name.clone(),
                    details: e.to_string(),
                })?;

            let mut chat_tools = Vec::with_capacity(mcp_tools.len());
            for mcp_tool in &mcp_tools {
                let def = to_chat_tool(&entry.name, mcp_tool);
                routes.insert(
                    def.function.name.clone(),
                    Route {
                        server_id: entry.id.clone(),
                        tool_name: mcp_tool.name.clone(),
                    },
                );
                chat_tools.push(def);
            }

            tracing::info!(
                server = %entry.name,
                id = %entry.id,
                tool_count = chat_tools.len(),
                "MCP server started"
            );

            servers.insert(
                entry.id.clone(),
                PoolServer {
                    config: entry.clone(),
                    client: Mutex::new(client),
                    chat_tools,
                },
            );
        }

        Ok(Self { servers, routes })
    }

    /// All prefixed tool names across all servers (for `on_tool_call` filtering).
    pub fn all_tool_names(&self) -> Vec<String> {
        self.routes.keys().cloned().collect()
    }

    /// Chat-facing tool definitions owned by one server.
    pub fn tools_for_server(&self, server_id: &str) -> &[ChatToolDefinition] {
        self.servers
            .get(server_id)
            .map(|s| s.chat_tools.as_slice())
            .unwrap_or(&[])
    }

    /// Resolved configs (for gating in Phase 4).
    pub fn server_configs(&self) -> Vec<&ResolvedServer> {
        self.servers.values().map(|s| &s.config).collect()
    }

    pub fn has_server(&self, server_id: &str) -> bool {
        self.servers.contains_key(server_id)
    }

    /// Resolve a prefixed chat tool name to its routing target.
    pub fn route(&self, prefixed_name: &str) -> Option<&Route> {
        self.routes.get(prefixed_name)
    }

    /// Execute a tool call on the owning server. Serialized per server (AD-5);
    /// different servers run concurrently.
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<ToolResult, PoolError> {
        let server = self
            .servers
            .get(server_id)
            .ok_or_else(|| PoolError::UnknownServer(server_id.to_string()))?;

        tracing::debug!(server = %server.config.name, tool = %tool_name, "calling MCP tool");
        let client = server.client.lock().await;
        client
            .call_tool(tool_name, arguments_json)
            .await
            .map_err(|e| PoolError::Call {
                server: server.config.name.clone(),
                tool: tool_name.to_string(),
                details: e.to_string(),
            })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("failed to spawn MCP server '{server}': {details}")]
    Spawn { server: String, details: String },
    #[error("failed to list tools from MCP server '{server}': {details}")]
    ListTools { server: String, details: String },
    #[error("unknown MCP server id '{0}'")]
    UnknownServer(String),
    #[error("MCP call failed on server '{server}' tool '{tool}': {details}")]
    Call {
        server: String,
        tool: String,
        details: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_prefixed_names() {
        assert_eq!(split_prefixed("fs:read_file"), Some(("fs", "read_file")));
        assert_eq!(split_prefixed("fs:a:b"), Some(("fs", "a:b"))); // first colon
        assert_eq!(split_prefixed("noprefix"), None);
        assert_eq!(split_prefixed(":x"), None);
        assert_eq!(split_prefixed("x:"), None);
    }

    #[test]
    fn maps_mcp_tool_to_prefixed_chat_tool() {
        let mcp = McpToolDefinition {
            name: "read_file".to_string(),
            description: "Read".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
        };
        let chat = to_chat_tool("filesystem", &mcp);
        assert_eq!(chat.tool_type, "function");
        assert_eq!(chat.function.name, "filesystem:read_file");
        assert_eq!(chat.function.description, "Read");
        assert_eq!(
            chat.function.parameters,
            serde_json::json!({ "type": "object" })
        );
    }
}
