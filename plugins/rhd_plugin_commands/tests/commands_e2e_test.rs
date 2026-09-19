//! End-to-end tests for the slash-commands plugin.
//!
//! Drives a real chat server + real `ai_completions` plugin + the commands
//! plugin against the mock AI provider. Proves the full feature across every
//! plugin boundary: `ai_completions:preDrainQueue` emission → command parsing →
//! positional queue rewrites → drain ordering.
//!
//! Startup ordering (KNOWN ISSUE A): the commands plugin MUST be registered
//! BEFORE the AI completions plugin creates its `PluginsMonitor`. That monitor
//! snapshots the plugin list once at creation (`PluginsMonitor::new` calls
//! `getPlugins` and registers a *local* plugins-list callback, but never calls
//! `subscribePluginsList` on the server, so no later `pluginRegistered` event
//! ever reaches it). A plugin that registers after the snapshot is therefore
//! never awaited by the ack-wait. Registering `commands` first makes the
//! coordination genuinely cover it and keeps these scenarios deterministic.
//!
//! `TestEnv` copies the structure of the ai_completions suites
//! (`pre_drain_queue_test.rs`, `tool_call_e2e_test.rs`): ephemeral-port chat
//! server on an in-memory DB, a recording mock AI, and temp-dir config files —
//! extended with a commands config + prompt files.

use std::future::Future;
use std::time::{Duration, Instant};

use rhd_chat_api::{
    AddMessageParams, AddQueueMessageParams, CreateChatParams, GetChatParams, GetPluginsParams,
    Message, RegisterPluginParams,
};
use rhd_chat_client::ChatClient;
use rhd_mock_ai_provider::{MockAiProvider, MockAiResponse, RecordingListener};
use rhd_plugin_ai_completions::config::PluginConfig as AiPluginConfig;
use rhd_plugin_ai_completions::plugin as ai_plugin;
use rhd_plugin_commands::config::{load_config as load_commands_config, CommandRegistry};
use rhd_plugin_commands::plugin as cmd_plugin;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Canonical prompt-file contents referenced by the test commands config.
/// Written without a trailing newline so prompt content is asserted verbatim.
const PROMPT_ONE: &str = "PROMPT_ONE";
const PROMPT_TWO: &str = "PROMPT_TWO";
const SYSTEM_PROMPT: &str = "SYSTEM_PROMPT";

/// The canonical 4-command example (same shapes the Phase 3 config tests cover).
const COMMANDS_CONFIG: &str = r#"
commands:
  tags_example:
    type: chat_tags
    add: [mcp:common]
    remove: [pause]
  prompt_example: ./commands/prompt.md
  system_prompt_example:
    type: prompt
    role: system
    prompt: ./systemPrompts/warhammer.md
  multi_example:
    - type: message_tags
      add: [some_tag]
    - ./commands/prompt.md
    - ./commands/another_prompt.md
"#;

/// Initialize tracing for tests (call at the start of each test).
fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Full-stack test environment: chat server, mock AI, and loaded configs for
/// both plugins.
struct TestEnv {
    chat_server_port: u16,
    _mock_ai: MockAiProvider,
    listener: RecordingListener,
    ai_config: AiPluginConfig,
    commands_config: CommandRegistry,
    _tmp: TempDir,
}

impl TestEnv {
    async fn new() -> Self {
        // Ephemeral-port chat server backed by an in-memory DB.
        let chat_config = rhd_chat_server::config::Config {
            host: "127.0.0.1".to_string(),
            port: 0,
            db_path: ":memory:".to_string(),
            clear_pending_acks: false,
        };
        let (chat_server_port, _server_handle) = rhd_chat_server::server::start(chat_config)
            .await
            .expect("Failed to start chat server");

        // Recording mock AI provider on an ephemeral port.
        let listener = RecordingListener::new();
        let mock_ai = MockAiProvider::start(listener.clone())
            .await
            .expect("Failed to start mock AI provider");

        // One temp dir holds everything: AI config + credentials, the commands
        // config, and the prompt markdown files it references.
        let tmp = TempDir::new().expect("Failed to create temp dir");
        let ai_config_path = tmp.path().join("ai-config.yaml");
        let creds_path = tmp.path().join("credentials.yaml");
        std::fs::write(&creds_path, "testApiKey: test-api-key-12345\n")
            .expect("Failed to write credentials");
        let ai_config_content = format!(
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
            creds_path.to_str().unwrap(),
            mock_ai.base_url()
        );
        std::fs::write(&ai_config_path, ai_config_content).expect("Failed to write AI config");

        // Commands config + prompt files (paths in the YAML resolve relative to
        // the config file, i.e. the temp dir root).
        std::fs::create_dir_all(tmp.path().join("commands")).expect("mkdir commands");
        std::fs::create_dir_all(tmp.path().join("systemPrompts")).expect("mkdir systemPrompts");
        std::fs::write(tmp.path().join("commands/prompt.md"), PROMPT_ONE)
            .expect("write prompt.md");
        std::fs::write(tmp.path().join("commands/another_prompt.md"), PROMPT_TWO)
            .expect("write another_prompt.md");
        std::fs::write(tmp.path().join("systemPrompts/warhammer.md"), SYSTEM_PROMPT)
            .expect("write warhammer.md");
        let commands_config_path = tmp.path().join("commands-config.yaml");
        std::fs::write(&commands_config_path, COMMANDS_CONFIG).expect("write commands config");

        let ai_config =
            rhd_plugin_ai_completions::config::load_config(ai_config_path.to_str().unwrap())
                .expect("Failed to load AI config");
        let commands_config = load_commands_config(commands_config_path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("commands config must load: {e}"));

        Self {
            chat_server_port,
            _mock_ai: mock_ai,
            listener,
            ai_config,
            commands_config,
            _tmp: tmp,
        }
    }

    fn chat_server_url(&self) -> String {
        format!("ws://127.0.0.1:{}/", self.chat_server_port)
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

/// Poll `check` every 100 ms until it returns true or `limit` elapses.
/// Returns whether the condition was met. Preferred over fixed sleeps for
/// assertion targets (Implementation Note 1).
async fn wait_until<F, Fut>(limit: Duration, mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = Instant::now() + limit;
    loop {
        if check().await {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

/// Connect a plain (non-plugin) observer client used to drive the API.
///
/// Deliberately NOT registered as a plugin: a registered plugin would be
/// snapshotted by the AI completions monitor and awaited on every event, so
/// leaving it unregistered keeps the awaited set to exactly the commands plugin.
async fn connect(url: &str) -> ChatClient {
    ChatClient::connect(url)
        .await
        .expect("Failed to connect client")
}

/// Spawn the commands plugin and block until its registration is observable via
/// `getPlugins`. MUST be called before `start_ai`.
async fn start_commands(env: &TestEnv) -> tokio::task::JoinHandle<()> {
    let url = env.chat_server_url();
    let registry = env.commands_config.clone();
    let handle = tokio::spawn(async move {
        // run_plugin only returns on fatal startup errors; it otherwise loops forever.
        let _ = cmd_plugin::run_plugin(&url, "commands", registry).await;
    });

    let probe = connect(&env.chat_server_url()).await;
    let registered = wait_until(Duration::from_secs(10), || async {
        list_plugin_ids(&probe).await.iter().any(|id| id == "commands")
    })
    .await;
    assert!(registered, "commands plugin did not register in time");
    // The registration becomes visible right after `registerPlugin`; give the
    // plugin a moment to also reach its custom-event subscription (the 500 ms
    // registration delay the existing suites use).
    sleep(Duration::from_millis(500)).await;
    handle
}

/// Spawn the AI completions plugin and let it finish connecting, registering,
/// creating its monitor (which snapshots the already-registered commands
/// plugin), and subscribing to all chats.
fn start_ai(env: &TestEnv) -> tokio::task::JoinHandle<()> {
    let url = env.chat_server_url();
    let config = env.ai_config.clone();
    let handle = tokio::spawn(async move {
        let _ = ai_plugin::run_plugin(&url, "ai_completions", config).await;
    });
    handle
}

/// Register a client as a plugin (needed to add tools; used only to drive the
/// tool-loop parking in scenario 5). Returns the client for further use.
async fn register_as_plugin(client: &ChatClient, plugin_id: &str) {
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .expect("Failed to register client as plugin");
}

/// Fetch the full regular-message history of a chat (insertion/id order).
async fn get_history(client: &ChatClient, chat_id: i64) -> Vec<Message> {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat")
        .messages
}

/// Fetch a chat's tags.
async fn get_chat_tags(client: &ChatClient, chat_id: i64) -> Vec<String> {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat")
        .chat
        .tags
}

/// Current queue contents for a chat, in drain (position) order.
async fn get_queue(client: &ChatClient, chat_id: i64) -> Vec<Message> {
    client
        .get_queue_messages(rhd_chat_api::GetQueueMessagesParams { chat_id })
        .await
        .expect("Failed to get queue")
        .messages
}

async fn list_plugin_ids(client: &ChatClient) -> Vec<String> {
    client
        .get_plugins(GetPluginsParams {})
        .await
        .expect("Failed to get plugins")
        .plugins
        .into_iter()
        .map(|p| p.plugin_id)
        .collect()
}

/// Queue one message on a chat; returns its id.
async fn queue_message(client: &ChatClient, chat_id: i64, role: &str, content: &str) -> i64 {
    client
        .add_queue_message(AddQueueMessageParams {
            chat_id,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            before_message_id: None,
        })
        .await
        .expect("Failed to queue message")
        .message_id
}

/// Queue a `user`-role message; returns its id.
async fn queue_text(client: &ChatClient, chat_id: i64, content: &str) -> i64 {
    queue_message(client, chat_id, "user", content).await
}

async fn create_chat(client: &ChatClient, title: &str, tags: Vec<String>) -> i64 {
    client
        .create_chat(CreateChatParams {
            title: title.to_string(),
            tags,
        })
        .await
        .expect("Failed to create chat")
        .chat_id
}

fn has_tag(msg: &Message, tag: &str) -> bool {
    msg.tags.iter().any(|t| t == tag)
}

/// User-role contents, in history order.
fn user_contents(history: &[Message]) -> Vec<String> {
    history
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.clone())
        .collect()
}

fn assistant_replied(history: &[Message]) -> bool {
    history
        .iter()
        .any(|m| m.role == "assistant" && m.is_finished && !m.content.is_empty())
}

// ============================================================================
// Scenarios
// ============================================================================

/// 1. A command followed by text: prompt is inserted before the stripped text.
#[tokio::test]
async fn test_command_with_trailing_text() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Command + text", vec![]).await;
        queue_text(&client, chat_id, "/tags_example /prompt_example hello").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply after the queue drains");

        let history = get_history(&client, chat_id).await;
        assert_eq!(history.len(), 3, "history: {history:?}");
        assert_eq!(history[0].role, "user");
        assert_eq!(history[0].content, PROMPT_ONE);
        assert!(
            has_tag(&history[0], "commands:prompt:prompt_example"),
            "prompt message must carry its tag: {:?}",
            history[0].tags
        );
        assert_eq!(history[1].role, "user");
        assert_eq!(history[1].content, "hello");
        assert!(history[1].tags.is_empty(), "stripped message is tag-free");
        assert_eq!(history[2].role, "assistant");
        assert_eq!(history[2].content, "Test response");

        let tags = get_chat_tags(&client, chat_id).await;
        assert!(tags.iter().any(|t| t == "mcp:common"), "chat tags: {tags:?}");
        assert!(!tags.iter().any(|t| t == "pause"), "pause must be removed");

        assert!(get_queue(&client, chat_id).await.is_empty(), "queue drained");

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 2. A message containing only commands is deleted after its steps run.
#[tokio::test]
async fn test_command_only_message_is_removed() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Command only", vec![]).await;
        queue_text(&client, chat_id, "/tags_example /prompt_example").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply (drain had the prompt message)");

        let history = get_history(&client, chat_id).await;
        assert!(
            !history
                .iter()
                .any(|m| m.content == "/tags_example /prompt_example"),
            "command-only message must be gone: {history:?}"
        );
        // The prompt survived and sits at the (former) head of the drain.
        let prompt = history
            .iter()
            .find(|m| m.content == PROMPT_ONE)
            .expect("PROMPT_ONE must be present");
        assert_eq!(prompt.role, "user");
        assert!(has_tag(prompt, "commands:prompt:prompt_example"));

        let tags = get_chat_tags(&client, chat_id).await;
        assert!(tags.iter().any(|t| t == "mcp:common"), "chat tags: {tags:?}");
        assert!(!tags.iter().any(|t| t == "pause"), "pause must be removed");

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 3. Adjacent commands with no separating whitespace behave like whitespace-separated.
#[tokio::test]
async fn test_adjacent_commands_without_spaces() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Adjacent commands", vec![]).await;
        queue_text(&client, chat_id, "/tags_example/prompt_example hi").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply after the queue drains");

        let history = get_history(&client, chat_id).await;
        assert_eq!(history.len(), 3, "history: {history:?}");
        assert_eq!(history[0].content, PROMPT_ONE);
        assert!(has_tag(&history[0], "commands:prompt:prompt_example"));
        assert_eq!(history[1].role, "user");
        assert_eq!(history[1].content, "hi");
        assert_eq!(history[2].role, "assistant");
        assert_eq!(history[2].content, "Test response");

        let tags = get_chat_tags(&client, chat_id).await;
        assert!(tags.iter().any(|t| t == "mcp:common"), "chat tags: {tags:?}");

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 4. A multi command expands its steps in config order and tags the carrying message.
#[tokio::test]
async fn test_multi_command_expands_in_order() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Multi command", vec![]).await;
        queue_text(&client, chat_id, "/multi_example tail").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply after the queue drains");

        let history = get_history(&client, chat_id).await;
        assert_eq!(user_contents(&history), vec![PROMPT_ONE, PROMPT_TWO, "tail"]);
        assert_eq!(history.len(), 4, "3 user + 1 assistant: {history:?}");
        assert_eq!(history[3].role, "assistant");

        let tail = history
            .iter()
            .find(|m| m.content == "tail")
            .expect("tail message present");
        assert!(
            has_tag(tail, "some_tag"),
            "carrying message must keep the multi command's message tag: {:?}",
            tail.tags
        );

        assert!(get_queue(&client, chat_id).await.is_empty(), "queue drained");

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 5. Two queued command messages, both present before the single drain,
/// interleave prompt/text in queue order — the justification for positional
/// insertion over append. Made deterministic with tool-loop parking: while the
/// chat has an unresolved tool call, queued messages cannot trigger a drain, so
/// both messages are guaranteed to be in the same `preDrainQueue` snapshot.
#[tokio::test]
async fn test_two_queued_command_messages_keep_interleaved_order() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        // Request 1 (primer): a tool call that parks the loop.
        env.listener
            .push_response(MockAiResponse::stream_tool_call("get_weather", r#"{"city":"London"}"#));
        // Request 2 (after resolving, drains the two queued command messages).
        env.listener
            .push_response(MockAiResponse::stream_text("Done"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        register_as_plugin(&client, "commands_e2e_tool_owner").await;
        let chat_id = create_chat(&client, "Interleaved", vec![]).await;

        // Primer drives the chat into a tool loop (parked on an unresolved call).
        queue_text(&client, chat_id, "What's the weather?").await;
        let parked = wait_until(Duration::from_secs(15), || async {
            get_history(&client, chat_id)
                .await
                .iter()
                .any(|m| m.role == "assistant" && !m.tool_calls.is_empty())
        })
        .await;
        assert!(parked, "primer should produce an assistant tool call");

        let tool_call_id = get_history(&client, chat_id)
            .await
            .iter()
            .find_map(|m| m.tool_calls.first().map(|tc| tc.id.clone()))
            .expect("tool call id");

        // Parked: queue both command messages. No drain can happen meanwhile.
        queue_text(&client, chat_id, "/prompt_example one").await;
        queue_text(&client, chat_id, "/prompt_example two").await;

        // Resolve the tool → the queuedMessages trigger fires for both messages.
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "tool".to_string(),
                content: r#"{"temperature":"20C"}"#.to_string(),
                tool_call_id: Some(tool_call_id),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .expect("Failed to add tool result");

        let ok = wait_until(Duration::from_secs(15), || async {
            let history = get_history(&client, chat_id).await;
            get_queue(&client, chat_id).await.is_empty()
                && history
                    .iter()
                    .any(|m| m.role == "assistant" && m.content == "Done")
        })
        .await;
        assert!(ok, "both command messages must drain and the loop must finish");

        let history = get_history(&client, chat_id).await;
        assert_eq!(
            user_contents(&history),
            vec!["What's the weather?", PROMPT_ONE, "one", PROMPT_ONE, "two"],
            "interleaved prompt/text order must survive the drain: {history:?}"
        );

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 6. Unknown commands pass through verbatim; a recognized command before an
/// unknown one still runs its steps, and the unknown token is kept as text.
#[tokio::test]
async fn test_unknown_command_passes_through() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        // One response if the two messages coalesce; a second covers the split
        // case — the assertions hold either way.
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Unknown command", vec![]).await;
        queue_text(&client, chat_id, "/notacommand hello").await;
        queue_text(&client, chat_id, "/tags_example /notacommand x").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            let history = get_history(&client, chat_id).await;
            let contents = user_contents(&history);
            assistant_replied(&history)
                && contents
                    .iter()
                    .any(|c| c == "/notacommand hello")
                && contents.iter().any(|c| c == "/notacommand x")
        })
        .await;
        assert!(ok, "both messages must drain verbatim/partially-stripped");

        let history = get_history(&client, chat_id).await;
        assert_eq!(
            user_contents(&history),
            vec!["/notacommand hello", "/notacommand x"],
            "unknown text verbatim, recognized command stripped, order kept: {history:?}"
        );

        let tags = get_chat_tags(&client, chat_id).await;
        assert!(
            tags.iter().any(|t| t == "mcp:common"),
            "recognized tags_example ran on the second message: {tags:?}"
        );

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 7. Only `user`-role queued messages are scanned; a non-user message drains
/// verbatim and its command-looking text is never executed.
#[tokio::test]
async fn test_non_user_queue_messages_untouched() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        let _cmd = start_commands(&env).await;
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "Non-user message", vec![]).await;
        // A user message with no commands triggers the drain; keep it first so
        // the built request history stays provider-valid (assistant not leading).
        queue_text(&client, chat_id, "plain trigger").await;
        queue_message(&client, chat_id, "assistant", "/tags_example hidden").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply after the queue drains");

        let history = get_history(&client, chat_id).await;
        let hidden = history
            .iter()
            .find(|m| m.role == "assistant" && m.content == "/tags_example hidden")
            .expect("assistant-role queued message must drain verbatim");
        assert!(
            !has_tag(hidden, "commands:prompt:prompt_example"),
            "non-user message must not be rewritten"
        );

        let tags = get_chat_tags(&client, chat_id).await;
        assert!(
            !tags.iter().any(|t| t == "mcp:common"),
            "chat tags must be unchanged by the untouched assistant message: {tags:?}"
        );

        ai.abort();
        _cmd.abort();
    })
    .await
    .expect("test timed out");
}

/// 8. Regression guard: with ONLY the AI completions plugin running, the
/// unhandled `preDrainQueue` event completes immediately (no plugin awaits it)
/// and a command-shaped message drains verbatim.
#[tokio::test]
async fn test_no_commands_plugin_no_behavior_change() {
    init_tracing();
    timeout(Duration::from_secs(20), async {
        let env = TestEnv::new().await;
        env.listener
            .push_response(MockAiResponse::stream_text("Test response"));

        // AI completions only — no commands plugin started.
        let ai = start_ai(&env);
        sleep(Duration::from_millis(500)).await;

        let client = connect(&env.chat_server_url()).await;
        let chat_id = create_chat(&client, "No commands plugin", vec![]).await;
        queue_text(&client, chat_id, "/tags_example x").await;

        let ok = wait_until(Duration::from_secs(15), || async {
            assistant_replied(&get_history(&client, chat_id).await)
        })
        .await;
        assert!(ok, "assistant should reply even with no commands plugin");

        let history = get_history(&client, chat_id).await;
        assert!(
            history
                .iter()
                .any(|m| m.role == "user" && m.content == "/tags_example x"),
            "message must drain verbatim (untouched): {history:?}"
        );
        // No commands plugin → no prompt inserted, no chat tag applied.
        assert_eq!(user_contents(&history), vec!["/tags_example x"]);
        assert!(
            !get_chat_tags(&client, chat_id)
                .await
                .iter()
                .any(|t| t == "mcp:common"),
            "chat tags must be unchanged without the commands plugin"
        );

        ai.abort();
    })
    .await
    .expect("test timed out");
}
