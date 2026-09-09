//! Shared harness for the MCP plugin integration tests.
//!
//! Each integration-test binary compiles its own copy of this module, so
//! helpers that are unused in a given binary are expected — hence the
//! crate-wide `dead_code` allowance below.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures_util::StreamExt;
use rhd_chat_api::{
    AssistantMessageWithToolCallsData, CreateChatParams, FunctionCall, GetChatParams, Message,
    RegisterPluginParams, ToolCall,
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
use rhd_plugin_mcp::status::McpStatusTracker;

pub fn stub_cmd() -> String {
    env!("CARGO_BIN_EXE_mcp_stub_server").to_string()
}

pub fn config_with(server_name: &str, register_on_tag: Option<&str>) -> PluginConfig {
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

/// Multi-server config: one entry per `(name, cmd)`, id defaults to name.
pub fn config_multi(servers: &[(&str, &str)]) -> PluginConfig {
    PluginConfig {
        servers: servers
            .iter()
            .map(|(name, cmd)| ResolvedServer {
                id: name.to_string(),
                name: name.to_string(),
                cmd: cmd.to_string(),
                args: vec![],
                cwd: None,
                env: HashMap::new(),
                register_on_tag: None,
            })
            .collect(),
    }
}

/// Start a test server and return the port.
pub async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
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
pub async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
}

pub fn chat_state(chat_id: i64, tags: Vec<String>) -> ChatState {
    ChatState {
        chat_id,
        messages: vec![],
        queued_messages_count: 0,
        tags,
        version: 1,
    }
}

pub fn new_regs() -> ChatRegistrations {
    Arc::new(RwLock::new(HashMap::new()))
}

pub fn tool_call_event(
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

pub async fn tool_messages(client: &ChatClient, chat_id: i64) -> Vec<Message> {
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

pub struct TestEnv {
    pub client: Arc<ChatClient>,
    pub pool: Arc<McpPool>,
    pub regs: ChatRegistrations,
    pub tracker: Arc<McpStatusTracker>,
    pub chat_id: i64,
    pub _server: tokio::task::JoinHandle<()>,
}

/// Start in-process server, register plugin, start pool, create chat,
/// and run tool registration for a chat with the given tags.
pub async fn setup_registered_chat(
    register_on_tag: Option<&str>,
    chat_tags: Vec<String>,
) -> TestEnv {
    let (port, server) = start_test_server().await;
    let client = Arc::new(connect_client(port).await);
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let (pool, reports) = McpPool::startup(&config_with("stub", register_on_tag)).await;
    let pool = Arc::new(pool);
    let tracker = Arc::new(McpStatusTracker::new(pool.server_ids()));
    tracker.record_startup(&reports).await;
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
        tracker,
        chat_id,
        _server: server,
    }
}
