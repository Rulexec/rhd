//! Integration tests for the `ai_completions:preDrainQueue` custom event.
//!
//! The plugin must emit `preDrainQueue` after `preRequest` acks and before draining
//! the queue (queuedMessages trigger only), and await all other plugins' acks.
//! Per-file `TestEnv` follows the existing pattern (see `tool_call_e2e_test.rs`).

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, AddQueueMessageParams, AddToolsParams, CreateChatParams,
    CustomEventData, FunctionDefinition, GetChatParams, Message, RegisterPluginParams,
    ToolDefinition,
};
use rhd_chat_client::ChatClient;
use rhd_mock_ai_provider::{MockAiProvider, MockAiResponse, RecordingListener};
use rhd_plugin_ai_completions::config::PluginConfig;
use rhd_plugin_ai_completions::plugin;
use tempfile::NamedTempFile;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing for tests (call at the start of each test).
fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Test environment with mock servers (recording listener for tool-call flows).
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
        std::io::Write::write_all(&mut creds_file, b"testApiKey: test-api-key-12345\n")
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
        let config =
            rhd_plugin_ai_completions::config::load_config(config_file.path().to_str().unwrap())
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

/// Connect, register as a plugin, and stream custom events to a receiver,
/// acknowledging each one. Events are forwarded to the channel BEFORE their ack,
/// so a recorded event causally precedes any post-drain chat state.
///
/// IMPORTANT: must be called before the AI completions plugin starts — its
/// `PluginsMonitor` snapshots the plugin list at creation (no `subscribePluginsList`),
/// so only already-registered plugins are awaited.
async fn spawn_observer(
    url: &str,
    plugin_id: &str,
) -> (Arc<ChatClient>, mpsc::UnboundedReceiver<CustomEventData>) {
    let client = Arc::new(
        ChatClient::connect(url)
            .await
            .expect("Failed to connect observer"),
    );
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .expect("Failed to register observer plugin");

    let (tx, rx) = mpsc::unbounded_channel();
    let ack_client = Arc::clone(&client);
    // The token is auto-cancelled on drop (test end) — no explicit cleanup needed.
    let _token = client.on_custom_event(move |event| {
        let client = Arc::clone(&ack_client);
        let tx = tx.clone();
        async move {
            let _ = tx.send(event.clone());
            if let Err(e) = client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                })
                .await
            {
                tracing::error!(error = %e, "observer failed to acknowledge event");
            }
        }
    });

    (client, rx)
}

/// Fetch the regular (non-queued) messages of a chat.
async fn fetch_messages(client: &ChatClient, chat_id: i64) -> Vec<Message> {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat")
        .messages
}

/// Spawn the AI completions plugin ("test_plugin") in the background.
fn spawn_plugin(env: &TestEnv) -> tokio::task::JoinHandle<()> {
    let url = env.chat_server_url();
    let config = env.config.clone();
    tokio::spawn(async move {
        // run_plugin only returns on fatal startup errors; chats are served until aborted.
        let _ = plugin::run_plugin(&url, "test_plugin", config).await;
    })
}

/// Create a chat and queue one user message on it; returns the chat id.
async fn queue_user_message(client: &ChatClient, title: &str, content: &str) -> i64 {
    let chat_id = client
        .create_chat(CreateChatParams {
            title: title.to_string(),
            tags: vec![],
        })
        .await
        .expect("Failed to create chat")
        .chat_id;
    client
        .add_queue_message(AddQueueMessageParams {
            chat_id,
            role: "user".to_string(),
            content: content.to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            before_message_id: None,
        })
        .await
        .expect("Failed to queue message");
    chat_id
}

/// Poll `get_chat` every 200 ms until `condition` holds for its messages.
async fn wait_for_condition<F>(
    client: &ChatClient,
    chat_id: i64,
    limit_secs: u64,
    mut condition: F,
) -> Option<Vec<Message>>
where
    F: FnMut(&[Message]) -> bool,
{
    for _ in 0..(limit_secs * 5) {
        let messages = fetch_messages(client, chat_id).await;
        if condition(&messages) {
            return Some(messages);
        }
        sleep(Duration::from_millis(200)).await;
    }
    None
}

/// Drain every event already delivered to the observer, ordered by server timestamp.
fn collect_events(rx: &mut mpsc::UnboundedReceiver<CustomEventData>) -> Vec<CustomEventData> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events.sort_by_key(|e| e.created_at);
    events
}

/// Extract the `triggerReason` from a custom event's `additional` JSON payload.
fn trigger_reason_of(event: &CustomEventData) -> String {
    event
        .additional
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| v["triggerReason"].as_str().map(str::to_string))
        .unwrap_or_default()
}

#[tokio::test]
async fn test_pre_drain_queue_emitted_on_queued_trigger() {
    init_tracing();
    timeout(Duration::from_secs(30), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        // Observer registers first so the plugin's monitor awaits its acks.
        let (client, mut rx) = spawn_observer(&env.chat_server_url(), "observer_plugin").await;
        let plugin_handle = spawn_plugin(&env);
        sleep(Duration::from_millis(500)).await;

        let chat_id = queue_user_message(&client, "Pre-drain trigger", "/hello").await;

        // The assistant reply lands only after the drain, which is gated on the
        // observer's preDrainQueue ack — so all events are already recorded here.
        let messages = wait_for_condition(&client, chat_id, 20, |msgs| {
            msgs.iter()
                .any(|m| m.role == "assistant" && m.is_finished && !m.content.is_empty())
        })
        .await
        .expect("Assistant reply should arrive after the queue drain");

        let events = collect_events(&mut rx);
        let names: Vec<&str> = events.iter().map(|e| e.event_name.as_str()).collect();
        assert_eq!(
            names,
            vec!["ai_completions:preRequest", "ai_completions:preDrainQueue"],
            "preRequest must be followed by preDrainQueue"
        );
        assert_eq!(events[0].chat_id, Some(chat_id.to_string()));
        assert_eq!(events[1].chat_id, Some(chat_id.to_string()));
        assert_eq!(trigger_reason_of(&events[1]), "queuedMessages");

        // The queued message was drained into history unchanged (no commands plugin).
        assert!(messages.iter().any(|m| m.role == "user" && m.content == "/hello"));

        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_pre_drain_queue_not_emitted_on_tool_loop() {
    init_tracing();
    timeout(Duration::from_secs(40), async {
        let env = TestEnv::new().await;

        // First response: streaming tool call; second: final text after the result.
        env.listener
            .push_response(MockAiResponse::stream_tool_call("get_weather", r#"{"city":"London"}"#));
        env.listener
            .push_response(MockAiResponse::stream_text("The weather in London is sunny."));

        // Observer registers first so the plugin's monitor awaits its acks.
        let (client, mut rx) = spawn_observer(&env.chat_server_url(), "observer_plugin").await;
        let plugin_handle = spawn_plugin(&env);
        sleep(Duration::from_millis(500)).await;

        let chat_id = client
            .create_chat(CreateChatParams {
                title: "Tool loop".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat")
            .chat_id;
        client
            .add_tools(AddToolsParams {
                chat_id,
                tools: vec![ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "get_weather".to_string(),
                        description: "Get the current weather for a city".to_string(),
                        parameters: serde_json::json!({"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}),
                    },
                }],
            })
            .await
            .expect("Failed to add tools");

        // Queued trigger → exactly one preDrainQueue for the whole flow.
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "What's the weather in London?".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                before_message_id: None,
            })
            .await
            .expect("Failed to queue message");

        // Wait for the assistant message declaring the tool call, then resolve it.
        let messages = wait_for_condition(&client, chat_id, 20, |msgs| {
            msgs.iter().any(|m| m.role == "assistant" && !m.tool_calls.is_empty())
        })
        .await
        .expect("Assistant message with tool call should appear");
        let tool_call_id = messages
            .iter()
            .find_map(|m| m.tool_calls.first().map(|tc| tc.id.clone()))
            .expect("tool call id");

        client
            .add_message(AddMessageParams {
                chat_id,
                role: "tool".to_string(),
                content: r#"{"temperature":"20C","condition":"sunny"}"#.to_string(),
                tool_call_id: Some(tool_call_id),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .expect("Failed to add tool result");

        // Wait for the final assistant reply produced by the tool-loop continuation.
        wait_for_condition(&client, chat_id, 20, |msgs| {
            msgs.iter().any(|m| m.role == "assistant" && m.tool_calls.is_empty() && m.is_finished && !m.content.is_empty())
        })
        .await
        .expect("Final assistant reply should arrive after the tool loop");

        let events = collect_events(&mut rx);
        let pre_requests: Vec<&CustomEventData> = events.iter().filter(|e| e.event_name == "ai_completions:preRequest").collect();
        let drain_events: Vec<&CustomEventData> = events.iter().filter(|e| e.event_name == "ai_completions:preDrainQueue").collect();

        assert_eq!(pre_requests.len(), 2, "one preRequest per trigger");
        assert_eq!(trigger_reason_of(pre_requests[0]), "queuedMessages");
        assert_eq!(trigger_reason_of(pre_requests[1]), "toolLoopContinuation");

        // Exactly one preDrainQueue in the whole flow (from the queued trigger only),
        // and none at all attached to the tool-loop continuation.
        assert_eq!(drain_events.len(), 1, "preDrainQueue must not fire on tool-loop continuation");
        assert_eq!(trigger_reason_of(drain_events[0]), "queuedMessages");
        assert!(pre_requests[0].created_at < drain_events[0].created_at);
        assert!(drain_events[0].created_at < pre_requests[1].created_at);

        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}

#[tokio::test]
async fn test_drain_waits_for_pre_drain_queue_acks() {
    init_tracing();
    timeout(Duration::from_secs(30), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Delayed response"));

        // Observer that acks everything immediately EXCEPT preDrainQueue, which it
        // holds until the test acks manually. It registers BEFORE the plugin starts
        // so the plugin's monitor awaits its acks.
        let client = Arc::new(
            ChatClient::connect(&env.chat_server_url())
                .await
                .expect("Failed to connect observer"),
        );
        client
            .register_plugin(RegisterPluginParams {
                plugin_id: "ack_hold".to_string(),
            })
            .await
            .expect("Failed to register observer plugin");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let ack_client = Arc::clone(&client);
        let _token = client.on_custom_event(move |event| {
            let client = Arc::clone(&ack_client);
            let tx = tx.clone();
            async move {
                let held = event.event_name == "ai_completions:preDrainQueue";
                let _ = tx.send(event.clone());
                if !held {
                    if let Err(e) = client
                        .ack_custom_event(AckCustomEventParams {
                            event_id: event.event_id,
                            is_rejected: None,
                        })
                        .await
                    {
                        tracing::error!(error = %e, "observer failed to acknowledge event");
                    }
                }
            }
        });

        // Start plugin in background
        let plugin_handle = spawn_plugin(&env);
        sleep(Duration::from_millis(500)).await;

        let chat_id = queue_user_message(&client, "Held drain", "Hello").await;

        // Wait until the held preDrainQueue event reaches the observer.
        let held_event = timeout(Duration::from_secs(10), async {
            loop {
                let event = rx.recv().await.expect("observer channel closed");
                if event.event_name == "ai_completions:preDrainQueue" {
                    return event;
                }
            }
        })
        .await
        .expect("preDrainQueue should be emitted (preRequest is acked immediately)");

        // While the ack is held (well inside the 30 s timeout), nothing drains and
        // no AI request is made.
        sleep(Duration::from_secs(2)).await;
        let messages = fetch_messages(&client, chat_id).await;
        assert!(messages.is_empty(), "queue must not drain while the preDrainQueue ack is held: {messages:?}");
        assert!(env.listener.get_requests().is_empty(), "no AI request may be sent while the ack is held");

        // Release the held ack — the drain and the request must now complete,
        // comfortably under the 30 s timeout (no chat parking).
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: held_event.event_id,
                is_rejected: None,
            })
            .await
            .expect("Failed to release preDrainQueue ack");

        let messages = wait_for_condition(&client, chat_id, 15, |msgs| {
            msgs.iter().any(|m| m.role == "assistant" && m.is_finished && !m.content.is_empty())
        })
        .await
        .expect("Assistant reply should land after the held ack is released");
        assert!(messages.iter().any(|m| m.role == "user" && m.content == "Hello"));
        assert_eq!(env.listener.get_requests().len(), 1);
        assert!(
            !messages.iter().any(|m| m.tags.iter().any(|t| t == "ai_completions:error")),
            "chat must not be parked: the wait finished well within the 30 s timeout"
        );

        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}
