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

/// Start a test server and return the port.
async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create test config with in-memory database
    let config = Config {
        host: "127.0.0.1".to_string(),
        port,
        db_path: ":memory:".to_string(),
        clear_pending_acks: false,
    };

    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path).unwrap());

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();

    // Create stream manager
    let stream_manager = Arc::new(StreamManager::new());

    // Bind TCP listener
    let listener = tokio::net::TcpListener::bind(&config.socket_addr())
        .await
        .unwrap();

    // Start server in background
    let handle = tokio::spawn(async move {
        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(result) => result,
                Err(_) => break,
            };

            let db = Arc::clone(&db);
            let subscription_manager = subscription_manager.clone();
            let plugin_registry = plugin_registry.clone();
            let stream_manager = stream_manager.clone();

            tokio::spawn(async move {
                match tokio_tungstenite::accept_async(stream).await {
                    Ok(ws_stream) => {
                        let (write, read) = ws_stream.split();
                        let _ = handle_connection(
                            read,
                            write,
                            db,
                            subscription_manager,
                            plugin_registry,
                            stream_manager,
                        )
                        .await;
                    }
                    Err(_) => {}
                }
            });
        }
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (port, handle)
}

/// Connect a client to the test server.
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
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let pool = Arc::new(
        McpPool::startup(&config_with("stub", Some("mcp:common")))
            .await
            .unwrap(),
    );
    let regs = new_regs();

    // Ineligible chat (no tag): nothing registered.
    let chat_a = client
        .create_chat(CreateChatParams {
            title: "a".into(),
            tags: vec![],
        })
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
        .create_chat(CreateChatParams {
            title: "b".into(),
            tags: vec!["mcp:common".into()],
        })
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
    let mut names: Vec<_> = tools
        .iter()
        .map(|t| t.tool.function.name.clone())
        .collect();
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
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let pool = Arc::new(McpPool::startup(&config_with("stub", None)).await.unwrap());
    let regs = new_regs();

    let chat = client
        .create_chat(CreateChatParams {
            title: "w".into(),
            tags: vec!["worktree:W2".into()],
        })
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
        .create_chat(CreateChatParams {
            title: "w1".into(),
            tags: vec!["worktree:W1".into()],
        })
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
    assert!(regs2
        .read()
        .await
        .get(&chat_w1)
        .map(|s| s.is_empty())
        .unwrap_or(true));
}

// ---------- tool-call execution ----------

fn tool_call_event(
    chat_id: i64,
    name: &str,
    args: &str,
    call_id: &str,
) -> AssistantMessageWithToolCallsData {
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
                function: FunctionCall {
                    name: name.into(),
                    arguments: args.into(),
                },
                tags: vec![],
            }],
        },
        chat_version: 2,
        tool_names: vec![name.into()],
    }
}

async fn tool_messages(client: &ChatClient, chat_id: i64) -> Vec<Message> {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .unwrap()
        .messages
        .into_iter()
        .filter(|m| m.role == "tool")
        .collect()
}

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
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let pool = Arc::new(
        McpPool::startup(&config_with("stub", register_on_tag))
            .await
            .unwrap(),
    );
    let regs = new_regs();
    let chat_id = client
        .create_chat(CreateChatParams {
            title: "t".into(),
            tags: chat_tags.clone(),
        })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(&client, &pool, &regs, None, &chat_state(chat_id, chat_tags))
        .await
        .unwrap();
    TestEnv {
        client,
        pool,
        regs,
        chat_id,
        _server: server,
    }
}

#[tokio::test]
async fn tool_call_roundtrip_pushes_result_and_dedups() {
    let env = setup_registered_chat(None, vec![]).await;

    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;

    let results = tool_messages(&env.client, env.chat_id).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(results[0].content, "echo: hello");

    // Duplicate guard: replaying the same event adds nothing.
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;
    assert_eq!(tool_messages(&env.client, env.chat_id).await.len(), 1);
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
