# Phase 3: MCP server pool and tool registry

## Overview

Implement `mcp_pool.rs`: at plugin startup, spawn and initialize every configured MCP server via `rhd_mcp_client::McpClient::connect(cmd, args, cwd, env)`, fetch each server's tool list, and build:

1. A **chat-facing tool registry**: each MCP tool mapped to an `rhd_chat_api::ToolDefinition` named `{name}:{tool}` (e.g. `filesystem:read_file`).
2. A **routing table**: prefixed tool name → `(server_id, bare tool name)` for call dispatch in Phase 5.
3. **Serialized access per server**: each `McpClient` wrapped in `tokio::sync::Mutex` because the stdio transport is a strict one-request/one-line-response protocol (AD-5 in the grand plan).

**Scope:**
- In: `plugins/rhd_plugin_mcp/src/mcp_pool.rs`, module registration in `lib.rs`.
- Out: chat interaction (Phase 4), tool-call handling (Phase 5), tests (Phase 6).

**Depends on:** Phase 2 (`ResolvedServer`, `PluginConfig`). Phase 1 recommended first (tracing in the crate) but not compile-blocking.

## Files to Create/Modify

### 1. `plugins/rhd_plugin_mcp/src/lib.rs`

**Modification:** add the module:

```rust
pub mod config;
pub mod mcp_pool;
pub mod plugin;
```

### 2. `plugins/rhd_plugin_mcp/src/mcp_pool.rs`

**Create** — full implementation:

```rust
//! MCP server pool: spawns configured servers, owns their clients, and
//! routes prefixed tool names to the owning server.

use std::collections::HashMap;

use rhd_chat_api::{FunctionDefinition, ToolDefinition as ChatToolDefinition};
use rhd_mcp_client::{McpClient, McpClientTrait, ToolDefinition as McpToolDefinition, ToolResult};
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
```

### 3. `plugins/rhd_plugin_mcp/Cargo.toml`

No change — `rhd_mcp_client` and `tokio` were added in Phase 2.

## Integration with existing code

- `McpClient::connect` already performs `initialize` + `notifications/initialized` and caches tools; the extra `list_tools()` call here re-fetches over the wire (cheap, one round-trip per server at startup). `McpClientTrait` must be in scope for `list_tools`/`call_tool` (imported above).
- `ToolResult { content, is_error, raw_response }` — Phase 5 forwards `content` as the chat tool message; `is_error` is informational (the model sees the text either way, matching existing MCP error-as-content behavior).

## Tests

Authored in Phase 6 (`plans/mcp/phase-6-tests.md`):
- `to_chat_tool` prefixing + schema mapping (pure).
- `split_prefixed` edge cases (pure).
- `McpPool::startup` + `route` + `call_tool` round-trip against the stub MCP server binary (integration).

## Implementation Notes

1. **Why `Mutex<McpClient>` per server:** `StdioTransport::send_request` writes one line then reads exactly one line while holding separate mutexes for stdin and reader. Two concurrent `call_tool`s on one client could interleave (writer A, writer B, reader A consumes B's response). Holding one mutex across the whole call serializes requests per server. Different servers are independent `McpClient`s, so they parallelize naturally.
2. **Route key is the prefixed name; value stores the server's `id`** (not `name`): `id` is the unique internal key (config validation guarantees it), while `name` is only the display prefix.
3. **HashMap iteration order is nondeterministic** — `server_configs()` order must not matter; gating (Phase 4) treats the set of eligible servers as unordered. `add_tools` receives tools grouped by newly-eligible servers; order within the batch is irrelevant.
4. **Fail-fast startup:** any spawn/initialize/list failure returns `Err` from `startup`; `main` exits non-zero. No partial-pool mode (per AD-4).
5. **Process cleanup:** `StdioTransport::Drop` best-effort `start_kill()`s the child. The pool lives for the whole process; on plugin exit (signal/`tokio::main` return) clients drop and children die. No explicit shutdown API needed.
6. **Notification desync caveat** (see Phase 1 note 4): servers that push unsolicited JSON-RPC notifications between responses will corrupt the line protocol. Documented in the plugin README (Phase 2).
7. **Do not hold a `tracing` span guard across `await`:** `span.enter()` + `.await` in the same future makes it non-`Send`, which breaks `tokio::spawn` in the Phase 5 handler dispatch. Use plain `tracing::debug!` with fields (as above) or `.instrument()` at the call site if richer spans are ever needed.

## Dependencies

- Requires Phase 2 (`config::ResolvedServer`, `PluginConfig`).
- Required by Phase 4 (pool passed into `run_plugin`; `server_configs`, `tools_for_server`, `all_tool_names`) and Phase 5 (`route`, `call_tool`).
