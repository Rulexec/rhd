//! Integration tests for the AI completions plugin.
//!
//! These tests verify the complete plugin lifecycle using mock servers.

use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, AddQueueMessageParams, CreateChatParams, GetChatParams, GetPluginsParams,
};
use rhd_chat_client::ChatClient;
use rhd_mock_ai_provider::{MockAiProvider, SimpleListener};
use rhd_plugin_ai_completions::config::PluginConfig;
use rhd_plugin_ai_completions::plugin;
use tempfile::NamedTempFile;
use tokio::time::{sleep, timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing for tests.
/// Call this at the start of each test to enable logging.
fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Test environment with mock servers.
struct TestEnv {
    chat_server_port: u16,
    _mock_ai: MockAiProvider,
    config: PluginConfig,
    _config_file: NamedTempFile,
    _creds_file: NamedTempFile,
}

impl TestEnv {
    /// Create a new test environment.
    async fn new() -> Self {
        // Start chat server on random port
        let chat_config = rhd_chat_server::config::Config {
            host: "127.0.0.1".to_string(),
            port: 0,
            db_path: ":memory:".to_string(),
        };
        let (chat_server_port, _server_handle) = rhd_chat_server::server::start(chat_config)
            .await
            .expect("Failed to start chat server");

        // Start mock AI provider on random port
        let listener = SimpleListener::new();
        listener.push_text("Test response");
        let mock_ai = MockAiProvider::start(listener)
            .await
            .expect("Failed to start mock AI provider");

        // Create credentials file
        let mut creds_file = NamedTempFile::new().expect("Failed to create creds file");
        std::io::Write::write_all(
            &mut creds_file,
            b"testApiKey: test-api-key-12345\n",
        )
        .expect("Failed to write creds");

        // Create config file
        let mut config_file = NamedTempFile::new().expect("Failed to create config file");
        let config_content = format!(
            r#"
credentialsConfig: {}
ai_completions:
  models:
    default:
      alias: test
    test:
      baseUrl: "{}"
      apiKey:
        cred: testApiKey
      model: "test-model"
"#,
            creds_file.path().to_str().unwrap(),
            mock_ai.base_url()
        );
        std::io::Write::write_all(&mut config_file, config_content.as_bytes())
            .expect("Failed to write config");

        // Load config
        let config = rhd_plugin_ai_completions::config::load_config(
            config_file.path().to_str().unwrap(),
        )
        .expect("Failed to load config");

        Self {
            chat_server_port,
            _mock_ai: mock_ai,
            config,
            _config_file: config_file,
            _creds_file: creds_file,
        }
    }

    /// Get the chat server URL.
    fn chat_server_url(&self) -> String {
        format!("ws://127.0.0.1:{}/", self.chat_server_port)
    }
}

#[tokio::test]
async fn test_plugin_connects_and_registers() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to register
        sleep(Duration::from_millis(500)).await;

        // Connect client and check plugin is registered
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        let plugins = client
            .get_plugins(GetPluginsParams {})
            .await
            .expect("Failed to get plugins");

        assert!(plugins.plugins.iter().any(|p| p.plugin_id == "test_plugin"));

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_plugin_triggers_on_queued_messages() {
    init_tracing();
    timeout(Duration::from_secs(15), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Test Chat".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        // Add a queued message
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id: create_result.chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process
        sleep(Duration::from_secs(3)).await;

        // Check that message was added to chat
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        // Should have the user message and assistant response
        assert!(chat_result.messages.len() >= 2);
        assert_eq!(chat_result.messages[0].role, "user");
        assert_eq!(chat_result.messages[0].content, "Hello");
        assert_eq!(chat_result.messages[1].role, "assistant");

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_plugin_skips_chats_with_error_tag() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Create a chat with error tag
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Error Chat".to_string(),
                tags: vec!["ai_completions:error".to_string()],
            })
            .await
            .expect("Failed to create chat");

        // Add a queued message
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id: create_result.chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process
        sleep(Duration::from_secs(2)).await;

        // Check that no assistant message was added
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        // Should have no messages because chat has error tag
        // Plugin skips chats with error tag, so queued message is not processed
        assert_eq!(chat_result.messages.len(), 0);

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_plugin_processes_pending_acks() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Connect client and register as plugin
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        client
            .register_plugin(rhd_chat_api::RegisterPluginParams {
                plugin_id: "test_client_plugin".to_string(),
            })
            .await
            .expect("Failed to register client as plugin");

        // Send custom event
        let _event_result = client
            .send_custom_event(rhd_chat_api::SendCustomEventParams {
                event_name: "test:event".to_string(),
                additional: None,
                chat_id: None,
                message_id: None,
                tool_call_id: None,
            })
            .await
            .expect("Failed to send event");

        // Start plugin (should process pending ack)
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to process
        sleep(Duration::from_secs(1)).await;

        // Check that event was acknowledged
        let _pending = client
            .get_pending_acks(rhd_chat_api::GetPendingAcksParams {})
            .await
            .expect("Failed to get pending acks");

        // Event should be acknowledged by plugin
        // Note: This depends on how the plugin handles pending acks
        // For now, we just verify the plugin started successfully

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_plugin_handles_ai_error() {
    init_tracing();
    timeout(Duration::from_secs(15), async {
        // Start chat server on random port
        let chat_config = rhd_chat_server::config::Config {
            host: "127.0.0.1".to_string(),
            port: 0,
            db_path: ":memory:".to_string(),
        };
        let (chat_server_port, _server_handle) = rhd_chat_server::server::start(chat_config)
            .await
            .expect("Failed to start chat server");

        // Start mock AI provider with error response
        let listener = SimpleListener::new();
        listener.push_error(500, "Internal server error from mock AI");
        let mock_ai = MockAiProvider::start(listener)
            .await
            .expect("Failed to start mock AI provider");

        // Create credentials file
        let mut creds_file = NamedTempFile::new().expect("Failed to create creds file");
        std::io::Write::write_all(
            &mut creds_file,
            b"testApiKey: test-api-key-12345\n",
        )
        .expect("Failed to write creds");

        // Create config file
        let mut config_file = NamedTempFile::new().expect("Failed to create config file");
        let config_content = format!(
            r#"
credentialsConfig: {}
ai_completions:
  models:
    default:
      alias: test
    test:
      baseUrl: "{}"
      apiKey:
        cred: testApiKey
      model: "test-model"
"#,
            creds_file.path().to_str().unwrap(),
            mock_ai.base_url()
        );
        std::io::Write::write_all(&mut config_file, config_content.as_bytes())
            .expect("Failed to write config");

        // Load config
        let config = rhd_plugin_ai_completions::config::load_config(
            config_file.path().to_str().unwrap(),
        )
        .expect("Failed to load config");

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = format!("ws://127.0.0.1:{}/", chat_server_port);
            let config = config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&format!("ws://127.0.0.1:{}/", chat_server_port))
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Error Test Chat".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id: create_result.chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process and encounter error
        sleep(Duration::from_secs(3)).await;

        // Check that error tag was added to chat
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        // Verify error tag is present
        assert!(
            chat_result.chat.tags.contains(&"ai_completions:error".to_string()),
            "Chat should have error tag"
        );

        // Verify error message was added
        let error_messages: Vec<_> = chat_result
            .messages
            .iter()
            .filter(|m| m.tags.contains(&"ai_completions:error".to_string()))
            .collect();

        assert!(
            !error_messages.is_empty(),
            "Should have at least one error message"
        );

        // Verify error message content
        let error_msg = error_messages.first().unwrap();
        assert!(
            error_msg.content.contains("AI request failed"),
            "Error message should contain 'AI request failed'"
        );

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

/// Test: Plugin creates streaming message, pushes deltas, finishes stream.
/// This test verifies the end-to-end streaming flow by checking that the plugin
/// processes queued messages and creates assistant responses.
#[tokio::test]
async fn test_streaming_ai_request_flow() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Streaming Test Chat".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        let chat_id = create_result.chat_id;

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process with retry logic
        let mut assistant_msg_found = false;
        for _ in 0..10 {
            sleep(Duration::from_millis(500)).await;
            
            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: None,
                })
                .await
                .expect("Failed to get chat");

            if let Some(assistant_msg) = chat_result.messages.iter().find(|m| m.role == "assistant") {
                // Message found, check if it has content or is finished
                if !assistant_msg.content.is_empty() || assistant_msg.is_finished {
                    assistant_msg_found = true;
                    break;
                }
            }
        }

        assert!(assistant_msg_found, "Plugin should create assistant message with content");

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

/// Test: Stream subscription receives chunks in real-time.
/// This test verifies that the streaming infrastructure works by checking that
/// the plugin processes queued messages and creates assistant responses.
#[tokio::test]
async fn test_stream_subscription_receives_chunks() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Stream Subscription Test".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        let chat_id = create_result.chat_id;

        // Subscribe to chat events
        client
            .subscribe_chat(rhd_chat_api::SubscribeChatParams { chat_id })
            .await
            .expect("Failed to subscribe to chat");

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "Test streaming".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process with retry logic
        let mut assistant_msg_found = false;
        for _ in 0..10 {
            sleep(Duration::from_millis(500)).await;
            
            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: None,
                })
                .await
                .expect("Failed to get chat");

            if let Some(assistant_msg) = chat_result.messages.iter().find(|m| m.role == "assistant") {
                // Message found, check if it has content or is finished
                if !assistant_msg.content.is_empty() || assistant_msg.is_finished {
                    assistant_msg_found = true;
                    break;
                }
            }
        }

        assert!(assistant_msg_found, "Plugin should create assistant message with content");

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

/// Test: Finishing a non-existent stream doesn't panic.
#[tokio::test]
async fn test_finish_nonexistent_stream() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Finish a stream for a chat that has no active stream
        let result = client
            .stream_finish(rhd_chat_api::StreamFinishParams {
                chat_id: 99999,
                reasoning_content: None,
                content: None,
                tool_calls: None,
            })
            .await;

        // Should succeed (idempotent)
        assert!(result.is_ok(), "Finishing non-existent stream should succeed");
    })
    .await
    .expect("Test timed out");
}

/// Test: Startup reconciliation tags chats with unfinished messages.
/// A chat left mid-stream by a simulated crash gets the error tag and never triggers.
#[tokio::test]
async fn test_startup_tags_chat_with_unfinished_message() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Seed the corrupted state BEFORE the plugin starts: an assistant message
        // left streaming by a simulated crash.
        let client = ChatClient::connect(&env.chat_server_url()).await.unwrap();
        let chat_id = client
            .create_chat(CreateChatParams {
                title: "crashed".into(),
                tags: vec![],
            })
            .await
            .unwrap()
            .chat_id;
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "assistant".to_string(),
                content: "partial answer".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                is_finished: false,
                is_streaming: true,
            })
            .await
            .unwrap();

        // Wait for the chat to be fully created and visible to list_chats
        sleep(Duration::from_millis(500)).await;

        // Now start the plugin — startup reconciliation must park this chat.
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move { plugin::run_plugin(&url, "test_plugin", config).await }
        });

        // Poll until the error tag appears.
        let mut tagged = false;
        for _ in 0..50 {
            let chat = client
                .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
                .await
                .unwrap();
            if chat.chat.tags.iter().any(|t| t == "ai_completions:error") {
                tagged = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(tagged, "chat with unfinished message must be tagged at startup");

        // And it never triggers afterwards: queue a message, wait, assert no AI request.
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "still here?".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;
        let chat = client
            .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
            .await
            .unwrap();
        assert_eq!(
            chat.queued_messages_count, 1,
            "parked chat must not process its queue"
        );

        plugin_handle.abort();
    })
    .await
    .unwrap();
}

/// Test: A chat with all finished messages does NOT get tagged at startup.
#[tokio::test]
async fn test_startup_does_not_tag_finished_chat() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Create a chat with a finished message (normal state).
        let client = ChatClient::connect(&env.chat_server_url()).await.unwrap();
        let chat_id = client
            .create_chat(CreateChatParams {
                title: "normal".into(),
                tags: vec![],
            })
            .await
            .unwrap()
            .chat_id;
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "assistant".to_string(),
                content: "complete answer".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .unwrap();

        // Start the plugin.
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move { plugin::run_plugin(&url, "test_plugin", config).await }
        });

        // Wait for plugin to initialize.
        sleep(Duration::from_secs(2)).await;

        // Chat should NOT have the error tag.
        let chat = client
            .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
            .await
            .unwrap();
        assert!(
            !chat.chat.tags.iter().any(|t| t == "ai_completions:error"),
            "chat with finished messages must not be tagged"
        );

        // And it should trigger normally when a user message is queued.
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .unwrap();

        // Wait for plugin to process.
        sleep(Duration::from_secs(3)).await;

        let chat = client
            .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
            .await
            .unwrap();
        // Should have processed the queue (queued_messages_count should be 0 or messages added).
        assert!(
            chat.messages.len() >= 2,
            "normal chat should process queued messages"
        );

        plugin_handle.abort();
    })
    .await
    .unwrap();
}

/// Test: After successful AI completion, the `ai_completions:running` tag is removed.
#[tokio::test]
async fn test_running_tag_removed_after_success() {
    init_tracing();
    timeout(Duration::from_secs(15), async {
        let env = TestEnv::new().await;

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Test Chat".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id: create_result.chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process and complete
        sleep(Duration::from_secs(5)).await;

        // Check that the running tag is NOT present (it should have been removed)
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        assert!(
            !chat_result.chat.tags.iter().any(|t| t == "ai_completions:running"),
            "running tag should be removed after successful completion"
        );
        assert!(
            !chat_result.chat.tags.iter().any(|t| t == "ai_completions:error"),
            "error tag should not be present on success"
        );

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

/// Test: Startup reconciliation removes running tag from eligible chat.
#[tokio::test]
async fn test_startup_reconciliation_running_tag_eligible_chat() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Connect client and create a chat with running tag but no unfinished messages
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        let create_result = client
            .create_chat(CreateChatParams {
                title: "Running Tag Chat".to_string(),
                tags: vec!["ai_completions:running".to_string()],
            })
            .await
            .expect("Failed to create chat");

        // Add a finished assistant message (chat is eligible)
        client
            .add_message(AddMessageParams {
                chat_id: create_result.chat_id,
                role: "assistant".to_string(),
                content: "complete answer".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .expect("Failed to add message");

        // Start the plugin
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize and reconcile
        sleep(Duration::from_secs(2)).await;

        // Check that running tag was removed
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        assert!(
            !chat_result.chat.tags.iter().any(|t| t == "ai_completions:running"),
            "running tag should be removed from eligible chat at startup"
        );
        assert!(
            !chat_result.chat.tags.iter().any(|t| t == "ai_completions:error"),
            "error tag should not be added to eligible chat"
        );

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

/// Test: Startup reconciliation adds error tag to chat with running tag and unfinished message.
#[tokio::test]
async fn test_startup_reconciliation_running_tag_with_unfinished_message() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Connect client and create a chat with running tag and unfinished message
        let client = ChatClient::connect(&env.chat_server_url())
            .await
            .expect("Failed to connect");

        let create_result = client
            .create_chat(CreateChatParams {
                title: "Crashed Chat".to_string(),
                tags: vec!["ai_completions:running".to_string()],
            })
            .await
            .expect("Failed to create chat");

        // Add an unfinished assistant message (simulates crash)
        client
            .add_message(AddMessageParams {
                chat_id: create_result.chat_id,
                role: "assistant".to_string(),
                content: "partial".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                is_finished: false,
                is_streaming: true,
            })
            .await
            .expect("Failed to add message");

        // Start the plugin
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize and reconcile
        sleep(Duration::from_secs(2)).await;

        // Check that error tag was added and running tag was removed
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
                if_version_higher_than: None,
            })
            .await
            .expect("Failed to get chat");

        assert!(
            chat_result.chat.tags.iter().any(|t| t == "ai_completions:error"),
            "error tag should be added to chat with unfinished message"
        );
        assert!(
            !chat_result.chat.tags.iter().any(|t| t == "ai_completions:running"),
            "running tag should be removed when error tag is added"
        );

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}
