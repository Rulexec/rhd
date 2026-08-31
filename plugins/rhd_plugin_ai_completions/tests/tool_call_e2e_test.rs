//! E2E test for tool call scenario in AI completions plugin.
//!
//! This test verifies the complete tool call flow:
//! 1. Create chat and register a tool
//! 2. Send user message
//! 3. Plugin calls AI provider, which returns assistant message with tool call
//! 4. Test provides tool call result
//! 5. Plugin continues tool loop, calls AI provider again
//! 6. AI provider returns assistant message without tool calls
//! 7. Test verifies final state

use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, AddQueueMessageParams, AddToolsParams, CreateChatParams,
    GetChatParams, GetToolsParams,
};
use rhd_chat_client::ChatClient;
use rhd_mock_ai_provider::{MockAiProvider, MockAiResponse, RecordingListener};
use rhd_plugin_ai_completions::config::PluginConfig;
use rhd_plugin_ai_completions::plugin;
use tempfile::NamedTempFile;
use tokio::time::{sleep, timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing for tests.
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
    listener: RecordingListener,
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
            clear_pending_acks: false,
        };
        let (chat_server_port, _server_handle) = rhd_chat_server::server::start(chat_config)
            .await
            .expect("Failed to start chat server");

        // Start mock AI provider on random port with recording listener
        let listener = RecordingListener::new();
        let mock_ai = MockAiProvider::start(listener.clone())
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
            listener,
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


/// Test: Complete tool call flow.
///
/// This test verifies:
/// 1. Plugin calls AI with tools registered
/// 2. AI returns tool call
/// 3. Test provides tool result
/// 4. Plugin continues loop and calls AI again
/// 5. AI returns final response without tool calls
#[tokio::test]
async fn test_tool_call_complete_flow() {
    init_tracing();
    timeout(Duration::from_secs(30), async {
        let env = TestEnv::new().await;

        // Configure mock responses:
        // First response: streaming tool call
        env.listener.push_response(MockAiResponse::stream_tool_call("get_weather", r#"{"city":"London"}"#));
        // Second response: streaming final text after tool result
        env.listener.push_response(MockAiResponse::stream_text("The weather in London is sunny and 20°C."));

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
                title: "Tool Call Test".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        let chat_id = create_result.chat_id;

        // Register client as a plugin (required to add tools)
        client
            .register_plugin(rhd_chat_api::RegisterPluginParams {
                plugin_id: "test_client_plugin".to_string(),
            })
            .await
            .expect("Failed to register client as plugin");

        // Start background task to acknowledge custom events
        // This simulates a real plugin that responds to events
        let ack_client = client.clone();
        let event_ack_token = ack_client.clone().on_custom_event(move |event| {
            let ack_client = ack_client.clone();
            async move {
                tracing::info!(
                    event_id = %event.event_id,
                    event_name = %event.event_name,
                    "Received custom event, acknowledging"
                );
                if let Err(e) = ack_client
                    .ack_custom_event(AckCustomEventParams {
                        event_id: event.event_id,
                        is_rejected: None,
                    })
                    .await
                {
                    tracing::error!(error = %e, "Failed to acknowledge event");
                }
            }
        });

        // Register a tool
        client
            .add_tools(AddToolsParams {
                chat_id,
                tools: vec![rhd_chat_api::ToolDefinition {
                    tool_type: "function".to_string(),
                    function: rhd_chat_api::FunctionDefinition {
                        name: "get_weather".to_string(),
                        description: "Get the current weather for a city".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "city": {
                                    "type": "string",
                                    "description": "The city name"
                                }
                            },
                            "required": ["city"]
                        }),
                    },
                }],
            })
            .await
            .expect("Failed to add tools");

        // Verify tool was registered
        let tools_result = client
            .get_tools(GetToolsParams { chat_id })
            .await
            .expect("Failed to get tools");
        assert_eq!(tools_result.tools.len(), 1);
        assert_eq!(tools_result.tools[0].tool.function.name, "get_weather");

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "What's the weather in London?".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process and get tool call
        let mut tool_call_found = false;
        let mut tool_call_id = String::new();
        for _ in 0..20 {
            sleep(Duration::from_millis(500)).await;

            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: None,
                })
                .await
                .expect("Failed to get chat");

            // Look for assistant message with tool calls
            for msg in &chat_result.messages {
                if msg.role == "assistant" && !msg.tool_calls.is_empty() {
                    tool_call_found = true;
                    tool_call_id = msg.tool_calls[0].id.clone();
                    tracing::info!(
                        "Found tool call: id={}, name={}",
                        tool_call_id,
                        msg.tool_calls[0].function.name
                    );
                    break;
                }
            }

            if tool_call_found {
                break;
            }
        }

        assert!(
            tool_call_found,
            "Plugin should create assistant message with tool call"
        );
        assert!(!tool_call_id.is_empty(), "Tool call should have an ID");

        // Provide tool result
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "tool".to_string(),
                content: r#"{"temperature":"20C","condition":"sunny"}"#.to_string(),
                tool_call_id: Some(tool_call_id.clone()),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .expect("Failed to add tool result");

        // Wait for plugin to continue tool loop and get final response
        let mut final_response_found = false;
        for _ in 0..20 {
            sleep(Duration::from_millis(500)).await;

            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: None,
                })
                .await
                .expect("Failed to get chat");

            // Look for final assistant message (without tool calls, with content)
            let assistant_messages: Vec<_> = chat_result
                .messages
                .iter()
                .filter(|m| m.role == "assistant")
                .collect();

            // Should have at least 2 assistant messages:
            // 1. First with tool call
            // 2. Second with final response
            if assistant_messages.len() >= 2 {
                let last_assistant = assistant_messages.last().unwrap();
                if last_assistant.tool_calls.is_empty()
                    && !last_assistant.content.is_empty()
                    && last_assistant.is_finished
                {
                    final_response_found = true;
                    tracing::info!(
                        "Found final response: {}",
                        last_assistant.content
                    );
                    break;
                }
            }
        }

        assert!(
            final_response_found,
            "Plugin should create final assistant message after tool loop"
        );

        // Verify the AI provider received 2 requests
        let requests = env.listener.get_requests();
        assert_eq!(
            requests.len(),
            2,
            "AI provider should receive 2 requests: initial and after tool result"
        );

        // Verify first request had tools
        let first_request = &requests[0];
        assert!(
            first_request.tools.is_some(),
            "First request should include tools"
        );
        let tools = first_request.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "get_weather");

        // Verify second request had the tool result in messages
        let second_request = &requests[1];
        assert!(
            second_request.messages.len() >= 3,
            "Second request should have user, assistant (with tool call), and tool messages"
        );

        // Cleanup
        event_ack_token.cancel();
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}
