# Phase 6: Tests

## Overview

Add all tests for the MCP plugin:

1. **Unit tests** (appended to existing modules): config parsing/validation/env resolution, gating truth table, pure pool helpers.
2. **Stub MCP server binary** (`src/bin/mcp_stub_server.rs`) speaking line-delimited JSON-RPC over stdio — used by pool and handler tests.
3. **Integration tests** (`tests/integration_test.rs`): pool startup/routing/calls against the stub; registration gating against the in-process chat server (harness copied from `plugins/rhd_plugin_system_prompt/tests/integration_test.rs`); tool-call round trip, duplicate guard, error-as-content path.

**Scope:** tests + stub bin + dev-dependencies only. No production logic changes unless a test exposes a bug (fix minimally and note it).

**Depends on:** Phases 1–5 (complete implementation).

## Files to Create/Modify

### 1. `plugins/rhd_plugin_mcp/Cargo.toml`

**Modification:** extend `[dev-dependencies]` (harness needs server + db + ws accept):

```toml
[dev-dependencies]
tempfile = "3"
rhd_chat_server = { path = "../../packages/rhd_chat_server" }
rhd_db = { path = "../../packages/rhd_db" }
tokio-tungstenite = { workspace = true }
futures-util = { workspace = true }
chrono = { workspace = true }
```

No `[[bin]]` entry needed: `src/bin/mcp_stub_server.rs` is auto-discovered by cargo and exposed to integration tests via `env!("CARGO_BIN_EXE_mcp_stub_server")`.

### 2. `plugins/rhd_plugin_mcp/src/bin/mcp_stub_server.rs`

**Create** — minimal MCP server: `initialize`, `tools/list` (tools `echo`, `fail`), `tools/call` (`echo` returns `echo: <input>`, `fail` returns a JSON-RPC error). Notifications (no `id`) get no response.

```rust
//! Test-only stub MCP server speaking JSON-RPC 2.0 over line-delimited stdio.

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        // Notifications carry no id → no response.
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "mcp-stub", "version": "0.1.0" }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": [
                    {
                        "name": "echo",
                        "description": "Echo the input",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "input": { "type": "string" } },
                            "required": ["input"]
                        }
                    },
                    {
                        "name": "fail",
                        "description": "Always fails",
                        "inputSchema": { "type": "object", "properties": {} }
                    }
                ] }
            }),
            "tools/call" => {
                let tool = msg
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                if tool == "fail" {
                    json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32603, "message": "boom" }
                    })
                } else {
                    let input = msg
                        .get("params")
                        .and_then(|p| p.get("arguments"))
                        .and_then(|a| a.get("input"))
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": { "content": [ { "type": "text", "text": format!("echo: {}", input) } ] }
                    })
                }
            }
            _ => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        };

        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}
```

### 3. Unit tests appended to `src/config.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_config(body: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", body).unwrap();
        f
    }

    #[test]
    fn parses_full_example_with_defaults() {
        std::env::set_var("MCP_TEST_ROOT_1", "/tmp/available");
        let f = write_config(
            r#"
mcp:
  - id: fs1
    name: filesystem
    cmd: npx
    args:
      - '-y'
      - '@modelcontextprotocol/server-filesystem'
      - env: MCP_TEST_ROOT_1
    cwd: /work/dir
    env:
      SOME_VAR: some-value
    registerOnTag: 'mcp:common'
  - name: search
    cmd: search-cmd
"#,
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.servers.len(), 2);

        let fs = &cfg.servers[0];
        assert_eq!(fs.id, "fs1");
        assert_eq!(fs.name, "filesystem");
        assert_eq!(
            fs.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/tmp/available"]
        );
        assert_eq!(fs.cwd.as_deref(), Some("/work/dir"));
        assert_eq!(fs.env.get("SOME_VAR").map(String::as_str), Some("some-value"));
        assert_eq!(fs.register_on_tag.as_deref(), Some("mcp:common"));

        // id defaults to name; args/cwd/env/registerOnTag optional
        let s = &cfg.servers[1];
        assert_eq!(s.id, "search");
        assert!(s.args.is_empty());
        assert!(s.cwd.is_none());
        assert!(s.env.is_empty());
        assert!(s.register_on_tag.is_none());
    }

    #[test]
    fn missing_env_var_fails_startup() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: npx
    args:
      - env: MCP_TEST_DEFINITELY_UNSET_VAR
"#,
        );
        let err = load_config(f.path().to_str().unwrap()).unwrap_err();
        assert!(matches!(err, ConfigError::EnvVarMissing { .. }));
        assert!(err.to_string().contains("MCP_TEST_DEFINITELY_UNSET_VAR"));
    }

    #[test]
    fn rejects_duplicate_names() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: a
  - name: fs
    cmd: b
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::DuplicateName(_)
        ));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let f = write_config(
            r#"
mcp:
  - id: x
    name: a
    cmd: c
  - id: x
    name: b
    cmd: c
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::DuplicateId(_)
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: a
    bogus: 1
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::Parse(_)
        ));
    }

    #[test]
    fn rejects_empty_server_list() {
        let f = write_config("mcp: []\n");
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::Validation(_)
        ));
    }
}
```

Note: tests using `set_var` must use uniquely-named variables (as above) to avoid cross-test interference under cargo's parallel harness.

### 4. Unit tests appended to `src/gating.rs`

Truth table (worktree filter × chat tags × registerOnTag):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedServer;
    use std::collections::HashMap;

    fn server(id: &str, register_on_tag: Option<&str>) -> ResolvedServer {
        ResolvedServer {
            id: id.to_string(),
            name: id.to_string(),
            cmd: "x".to_string(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            register_on_tag: register_on_tag.map(|s| s.to_string()),
        }
    }

    fn tags(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn worktree_gate_without_flag_rejects_any_worktree_tag() {
        assert!(worktree_gate(&tags(&[]), None));
        assert!(worktree_gate(&tags(&["mcp:common"]), None));
        assert!(!worktree_gate(&tags(&["worktree:W1"]), None));
    }

    #[test]
    fn worktree_gate_with_flag_requires_exact_tag() {
        assert!(worktree_gate(&tags(&["worktree:W1"]), Some("W1")));
        assert!(!worktree_gate(&tags(&["worktree:W2"]), Some("W1")));
        assert!(!worktree_gate(&tags(&[]), Some("W1")));
        assert!(!worktree_gate(&tags(&["worktree:W1x"]), Some("W1")));
    }

    #[test]
    fn server_gate_exact_tag() {
        assert!(server_gate(&tags(&["mcp:common"]), Some("mcp:common")));
        assert!(!server_gate(&tags(&["mcp:other"]), Some("mcp:common")));
        assert!(!server_gate(&tags(&[]), Some("mcp:common")));
        assert!(server_gate(&tags(&[]), None));
    }

    #[test]
    fn eligible_ids_compose_both_gates() {
        let plain = server("plain", None);
        let tagged = server("tagged", Some("mcp:common"));
        let servers = vec![&plain, &tagged];

        // No worktree filter: plain eligible everywhere except worktree-tagged chats;
        // tagged only with its tag.
        assert_eq!(eligible_server_ids(&servers, &tags(&[]), None), vec!["plain"]);
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["mcp:common"]), None),
            vec!["plain", "tagged"]
        );
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W1"]), None),
            Vec::<String>::new()
        );

        // With worktree filter: only exact worktree tag passes, and then per-server gate applies.
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W1"]), Some("W1")),
            vec!["plain"]
        );
        assert_eq!(
            eligible_server_ids(
                &servers,
                &tags(&["worktree:W1", "mcp:common"]),
                Some("W1")
            ),
            vec!["plain", "tagged"]
        );
        assert_eq!(
            eligible_server_ids(&servers, &tags(&["worktree:W2"]), Some("W1")),
            Vec::<String>::new()
        );
    }
}
```

### 5. Unit tests appended to `src/mcp_pool.rs`

```rust
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
        assert_eq!(chat.function.parameters, serde_json::json!({ "type": "object" }));
    }
}
```

### 6. `plugins/rhd_plugin_mcp/tests/integration_test.rs`

**Create.** Harness (`start_test_server`, `connect_client`) is the proven pattern from `plugins/rhd_plugin_system_prompt/tests/integration_test.rs` (in-process `rhd_chat_server` on an ephemeral port, `:memory:` db). Tests:

```rust
//! Integration tests for the MCP plugin: pool ↔ stub server, gating ↔ chat server.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures_util::StreamExt;
use rhd_chat_api::{
    AssistantMessageWithToolCallsData, CreateChatParams, FunctionCall, GetChatParams,
    GetToolsParams, Message, RegisterPluginParams, ToolCall,
};
use rhd_chat_client::{ChatClient, ChatState};
use rhd_chat_server::config::Config;
use rhd_chat_server::connection::handle_connection;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::streams::StreamManager;
use rhd_chat_server::subscriptions::new_shared_subscription_manager;
use rhd_db::ChatDb;
use tokio::sync::RwLock;

use rhd_plugin_mcp::config::{PluginConfig, ResolvedServer};
use rhd_plugin_mcp::mcp_pool::McpPool;
use rhd_plugin_mcp::plugin::{register_missing_tools, ChatRegistrations};
use rhd_plugin_mcp::tool_handler;

fn stub_cmd() -> String {
    env!("CARGO_BIN_EXE_mcp_stub_server").to_string()
}

fn config_with(server_name: &str, register_on_tag: Option<&str>) -> PluginConfig {
    PluginConfig {
        servers: vec![ResolvedServer {
            id: server_name.to_string(),
            name: server_name.to_string(),
            cmd: stub_cmd(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            register_on_tag: register_on_tag.map(|s| s.to_string()),
        }],
    }
}

async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    // ... identical to rhd_plugin_system_prompt/tests/integration_test.rs lines 19–87
}

async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
}

fn chat_state(chat_id: i64, tags: Vec<String>) -> ChatState {
    ChatState {
        chat_id,
        messages: vec![],
        queued_messages_count: 0,
        tags,
        version: 1,
    }
}

fn new_regs() -> ChatRegistrations {
    Arc::new(RwLock::new(HashMap::new()))
}

// ---------- pool ----------

#[tokio::test]
async fn pool_startup_lists_and_routes_stub_tools() {
    let pool = McpPool::startup(&config_with("stub", None)).await.unwrap();

    let mut names = pool.all_tool_names();
    names.sort();
    assert_eq!(names, vec!["stub:echo", "stub:fail"]);

    let route = pool.route("stub:echo").unwrap();
    assert_eq!(route.server_id, "stub");
    assert_eq!(route.tool_name, "echo");

    let result = pool
        .call_tool("stub", "echo", r#"{"input":"hi"}"#)
        .await
        .unwrap();
    assert_eq!(result.content, "echo: hi");
    assert_ne!(result.is_error, Some(true));

    assert!(pool.call_tool("stub", "fail", "{}").await.is_err());
}

#[tokio::test]
async fn pool_startup_fails_fast_on_bad_cmd() {
    let mut cfg = config_with("bad", None);
    cfg.servers[0].cmd = "/nonexistent/mcp-binary".to_string();
    assert!(McpPool::startup(&cfg).await.is_err());
}

// ---------- registration gating ----------

#[tokio::test]
async fn registers_tools_only_on_eligible_chats() {
    let (port, _h) = start_test_server().await;
    let client = connect_client(port).await;
    client
        .register_plugin(RegisterPluginParams { plugin_id: "mcp".into() })
        .await
        .unwrap();
    let pool = Arc::new(McpPool::startup(&config_with("stub", Some("mcp:common"))).await.unwrap());
    let regs = new_regs();

    // Ineligible chat (no tag): nothing registered.
    let chat_a = client
        .create_chat(CreateChatParams { title: "a".into(), tags: vec![] })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(&client, &pool, &regs, None, &chat_state(chat_a, vec![]))
        .await
        .unwrap();
    assert!(client
        .get_tools(GetToolsParams { chat_id: chat_a })
        .await
        .unwrap()
        .tools
        .is_empty());

    // Eligible chat: prefixed tool registered under this plugin id.
    let chat_b = client
        .create_chat(CreateChatParams { title: "b".into(), tags: vec!["mcp:common".into()] })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(
        &client,
        &pool,
        &regs,
        None,
        &chat_state(chat_b, vec!["mcp:common".into()]),
    )
    .await
    .unwrap();
    let tools = client
        .get_tools(GetToolsParams { chat_id: chat_b })
        .await
        .unwrap()
        .tools;
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().all(|t| t.plugin_id == "mcp"));
    let mut names: Vec<_> = tools.iter().map(|t| t.tool.function.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["stub:echo", "stub:fail"]);

    // Idempotent: second pass adds nothing.
    register_missing_tools(
        &client,
        &pool,
        &regs,
        None,
        &chat_state(chat_b, vec!["mcp:common".into()]),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .get_tools(GetToolsParams { chat_id: chat_b })
            .await
            .unwrap()
            .tools
            .len(),
        2
    );
}

#[tokio::test]
async fn worktree_filter_blocks_and_admits_chats() {
    let (port, _h) = start_test_server().await;
    let client = connect_client(port).await;
    client
        .register_plugin(RegisterPluginParams { plugin_id: "mcp".into() })
        .await
        .unwrap();
    let pool = Arc::new(McpPool::startup(&config_with("stub", None)).await.unwrap());
    let regs = new_regs();

    let chat = client
        .create_chat(CreateChatParams { title: "w".into(), tags: vec!["worktree:W2".into()] })
        .await
        .unwrap()
        .chat_id;

    // Plugin bound to W1: W2 chat ineligible.
    register_missing_tools(
        &client,
        &pool,
        &regs,
        Some("W1"),
        &chat_state(chat, vec!["worktree:W2".into()]),
    )
    .await
    .unwrap();
    assert!(client
        .get_tools(GetToolsParams { chat_id: chat })
        .await
        .unwrap()
        .tools
        .is_empty());

    // W1 chat eligible.
    let chat_w1 = client
        .create_chat(CreateChatParams { title: "w1".into(), tags: vec!["worktree:W1".into()] })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(
        &client,
        &pool,
        &regs,
        Some("W1"),
        &chat_state(chat_w1, vec!["worktree:W1".into()]),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .get_tools(GetToolsParams { chat_id: chat_w1 })
            .await
            .unwrap()
            .tools
            .len(),
        2
    );

    // Unbound plugin must skip worktree-tagged chats entirely.
    let regs2 = new_regs();
    register_missing_tools(
        &client,
        &pool,
        &regs2,
        None,
        &chat_state(chat_w1, vec!["worktree:W1".into()]),
    )
    .await
    .unwrap();
    assert!(regs2.read().await.get(&chat_w1).map(|s| s.is_empty()).unwrap_or(true));
}

// ---------- tool-call execution ----------

fn tool_call_event(chat_id: i64, name: &str, args: &str, call_id: &str) -> AssistantMessageWithToolCallsData {
    AssistantMessageWithToolCallsData {
        chat_id,
        message: Message {
            id: 1,
            chat_id,
            role: "assistant".into(),
            content: String::new(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![ToolCall {
                id: call_id.into(),
                call_type: "function".into(),
                function: FunctionCall { name: name.into(), arguments: args.into() },
                tags: vec![],
            }],
        },
        chat_version: 2,
        tool_names: vec![name.into()],
    }
}

async fn tool_messages(client: &ChatClient, chat_id: i64) -> Vec<Message> {
    client
        .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
        .await
        .unwrap()
        .messages
        .into_iter()
        .filter(|m| m.role == "tool")
        .collect()
}

#[tokio::test]
async fn tool_call_roundtrip_pushes_result_and_dedups() {
    let (port, _h) = start_test_server().await;
    let client = Arc::new(connect_client(port).await);
    client
        .register_plugin(RegisterPluginParams { plugin_id: "mcp".into() })
        .await
        .unwrap();
    let pool = Arc::new(McpPool::startup(&config_with("stub", None)).await.unwrap());
    let regs = new_regs();

    let chat_id = client
        .create_chat(CreateChatParams { title: "t".into(), tags: vec![] })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(&client, &pool, &regs, None, &chat_state(chat_id, vec![]))
        .await
        .unwrap();

    tool_handler::handle_tool_calls(
        Arc::clone(&client),
        Arc::clone(&pool),
        Arc::clone(&regs),
        tool_call_event(chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;

    let results = tool_messages(&client, chat_id).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(results[0].content, "echo: hello");

    // Duplicate guard: replaying the same event adds nothing.
    tool_handler::handle_tool_calls(
        Arc::clone(&client),
        Arc::clone(&pool),
        Arc::clone(&regs),
        tool_call_event(chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;
    assert_eq!(tool_messages(&client, chat_id).await.len(), 1);
}

#[tokio::test]
async fn mcp_error_becomes_tool_content() {
    let env = setup_registered_chat(None, vec![]).await;
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        tool_call_event(env.chat_id, "stub:fail", "{}", "call_err"),
    )
    .await;

    let results = tool_messages(&env.client, env.chat_id).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_err"));
    assert!(results[0].content.contains("MCP tool call failed"));
}

#[tokio::test]
async fn unregistered_server_and_foreign_tools_are_skipped() {
    let env = setup_registered_chat(None, vec![]).await;

    // Routed to our pool but server not registered for this chat (fresh regs).
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        new_regs(),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"x"}"#, "call_skip"),
    )
    .await;

    // Tool name we do not own at all.
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        tool_call_event(env.chat_id, "other:tool", "{}", "call_foreign"),
    )
    .await;

    assert!(tool_messages(&env.client, env.chat_id).await.is_empty());
}
```

**Shared setup helper** (add alongside `tool_call_event`/`tool_messages`; the roundtrip test may also be rewritten to use it):

```rust
struct TestEnv {
    client: Arc<ChatClient>,
    pool: Arc<McpPool>,
    regs: ChatRegistrations,
    chat_id: i64,
    _server: tokio::task::JoinHandle<()>,
}

/// Start in-process server, register plugin, start pool, create chat,
/// and run tool registration for a chat with the given tags.
async fn setup_registered_chat(register_on_tag: Option<&str>, chat_tags: Vec<String>) -> TestEnv {
    let (port, server) = start_test_server().await;
    let client = Arc::new(connect_client(port).await);
    client
        .register_plugin(RegisterPluginParams { plugin_id: "mcp".into() })
        .await
        .unwrap();
    let pool = Arc::new(
        McpPool::startup(&config_with("stub", register_on_tag))
            .await
            .unwrap(),
    );
    let regs = new_regs();
    let chat_id = client
        .create_chat(CreateChatParams { title: "t".into(), tags: chat_tags.clone() })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(&client, &pool, &regs, None, &chat_state(chat_id, chat_tags))
        .await
        .unwrap();
    TestEnv { client, pool, regs, chat_id, _server: server }
}
```

The server's `addMessage` only requires `toolCallId` to be present for `role: "tool"` (no cross-validation against an assistant message — verified in `packages/rhd_chat_server/src/handlers/message.rs`), so manually constructed events + direct handler invocation is a valid test path.

(The `start_test_server` body is copied verbatim from the system_prompt integration test; only the imports differ. Elided here to avoid duplication — the implementer pastes lines 19–87 of that file.)

## Verification

```bash
mise run test-cargo            # all unit + integration tests
cargo test -p rhd_plugin_mcp   # plugin-scoped
```

## Implementation Notes

1. **Stub as `src/bin/`** keeps it inside the crate (auto-built for `cargo test`, accessible via `CARGO_BIN_EXE_...`) without shipping it as a user-facing binary beyond a clearly test-scoped name.
2. **`handle_tool_calls` is called directly** instead of driving the full event pipeline: assistant tool-call messages are created via the streaming API (`streamFinish`), which belongs to `rhd_plugin_ai_completions`' domain. Testing the handler function with a constructed event covers the plugin's responsibility (route → execute → answer) without reimplementing the AI loop. The `on_tool_call` filter itself is client-library behavior already covered by existing tests.
3. **`register_missing_tools` takes a `ChatState`** precisely so gating is testable without the monitor; the monitor wiring is exercised by the existing pattern in other plugins.
4. **Pool tests spawn real child processes** — keep them few; each test's pool drops at scope end, killing children (`StdioTransport::Drop`).
5. **If `:memory:` SQLite + multiple connections** in the harness behaves oddly (shared-cache semantics), fall back to a `tempfile` db path like other suites do.

## Dependencies

- Requires Phases 1–5 complete.
- Nothing depends on this phase (Phase 7 docs may reference test coverage).
