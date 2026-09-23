//! Full-pipeline environment for the Phase 7 e2e suite: chat server (in-memory)
//! + one mock AI provider behind a [`RoutingListener`] + BOTH production
//! plugins (`rhd_plugin_ai_completions` and this one) spawned as in-process
//! tasks + a registered helper client that acknowledges every custom event
//! (otherwise ai_completions' `preRequest` / `preDrainQueue` ack-waits stall
//! on it) and serves as the test's "user / operator" connection.
//!
//! Bootstrap plumbing lives in [`super::bootstrap`] (adapted from
//! `rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs`), the polling
//! assertions in [`super::polling`].
//!
//! Startup discipline (registration visibility is NOT readiness — both
//! plugins wire their handlers AFTER `registerPlugin` returns):
//!
//! - ai_completions: a ping on the bootstrap SENTINEL chat must
//!   demonstrably reach the provider before `start*` returns — the plugin's
//!   chat state-change callback only exists after its startup reconcile, so
//!   scenario chats are created strictly after it proved reactivity.
//! - the sub-chat plugin: it must acknowledge a live probe `customEvent`
//!   (its ack subscription is installed after registration) AND register
//!   its three tools on a fresh sentinel state change (the monitor →
//!   per-chat-registration chain, which `run_plugin` wires before the
//!   tool-call subscription and the watcher hook).
//!
//! The sentinel lane is scripted EMPTY, so its in-flight provider requests
//! park in the mock forever and never interfere with scenario assertions
//! (all counts are per scenario marker; the sentinel owns its own).

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use rhd_chat_api::{
    AckCustomEventParams, AddQueueMessageParams, CreateChatParams, GetPluginsParams,
    GetToolsParams, Message, RegisterPluginParams, UpdateChatParams,
};
use rhd_chat_client::{CancellationToken, ChatClient};
use rhd_mock_ai_provider::MockAiProvider;
use tokio::time::{sleep, timeout};

use super::bootstrap::{
    ack_probe, init_tracing, start_chat_server, write_plugin_config, AI_PLUGIN_ID, SENTINEL_MARKER,
    SUB_CHAT_PLUGIN_ID, HELPER_PLUGIN_ID, WAIT_TIMEOUT,
};
use super::polling::find_subchat;
use super::routing::RoutingListener;

pub struct FullEnv {
    /// Registered helper connection: creates chats, queues messages, edits
    /// tags (operator actions), and reads state for assertions.
    pub client: Arc<ChatClient>,
    pub listener: RoutingListener,
    server_url: String,
    /// Bootstrap gate chat (see `wait_ai_ready` / `wait_sub_chat_ready`):
    /// scenario chats are only created after the sentinel proves both
    /// plugins react to chat state changes.
    pub sentinel_chat: i64,
    _ai_plugin: tokio::task::JoinHandle<()>,
    sub_plugin: Option<tokio::task::JoinHandle<()>>,
    _ack_token: CancellationToken,
    _mock_ai: MockAiProvider,
    _config_file: tempfile::NamedTempFile,
    _creds_file: tempfile::NamedTempFile,
}

impl FullEnv {
    /// The complete production-shaped pipeline.
    pub async fn start() -> Self {
        let mut env = Self::start_without_sub_chat().await;
        env.start_sub_chat_plugin().await;
        env
    }

    /// Everything except the sub-chat plugin (scenario 7's "plugin crashed
    /// and is not running yet" window). Prefer [`FullEnv::start`] elsewhere.
    pub async fn start_without_sub_chat() -> Self {
        init_tracing();
        let server_url = start_chat_server().await;

        let listener = RoutingListener::new();
        // Sentinel lane: kept EMPTY on purpose. Each gate ping produces one
        // provider request whose response the test never scripts — the
        // request parks in the mock (route queue empty, see
        // [`super::routing`]), which is the desired end state: the plugin
        // stays busy on the throwaway sentinel chat, never on scenario ones.
        listener.register(SENTINEL_MARKER, vec![]);
        let mock_ai = MockAiProvider::start(listener.clone())
            .await
            .expect("mock AI provider must start");

        let (config, config_file, creds_file) = write_plugin_config(&mock_ai.base_url());

        let ai_plugin = {
            let url = server_url.clone();
            tokio::spawn(async move {
                rhd_plugin_ai_completions::plugin::run_plugin(&url, AI_PLUGIN_ID, config)
                    .await
                    .expect("ai_completions plugin task failed");
            })
        };

        // Helper connection: register first, then the ack-all subscription,
        // so every event emitted from here on lands in its handler.
        let client = Arc::new(
            ChatClient::connect(&server_url)
                .await
                .expect("helper connect failed"),
        );
        client
            .register_plugin(RegisterPluginParams {
                plugin_id: HELPER_PLUGIN_ID.to_string(),
            })
            .await
            .expect("helper registration failed");
        let ack_client = Arc::clone(&client);
        let ack_token = client.on_custom_event(move |event| {
            let ack_client = Arc::clone(&ack_client);
            async move {
                if let Err(e) = ack_client
                    .ack_custom_event(AckCustomEventParams {
                        event_id: event.event_id,
                        is_rejected: None,
                    })
                    .await
                {
                    tracing::error!(error = %e, "helper failed to acknowledge event");
                }
            }
        });
        let mut env = Self {
            client,
            listener,
            server_url,
            sentinel_chat: -1,
            _ai_plugin: ai_plugin,
            sub_plugin: None,
            _ack_token: ack_token,
            _mock_ai: mock_ai,
            _config_file: config_file,
            _creds_file: creds_file,
        };
        env.wait_for_plugin_visible(AI_PLUGIN_ID).await;
        env.sentinel_chat = env
            .client
            .create_chat(CreateChatParams {
                title: "e2e bootstrap sentinel".to_string(),
                tags: vec![],
            })
            .await
            .expect("sentinel chat")
            .chat_id;
        env.wait_ai_ready().await;
        env
    }

    /// ai_completions readiness gate. `run_plugin` registers its chat
    /// state-change callback only AFTER the startup reconcile, so plugin
    /// REGISTRATION (getPlugins visibility) does not imply it REACTS to
    /// chats: queue pings on the sentinel chat until one demonstrably
    /// reaches the provider. A ping fired while the plugin was still
    /// wiring its callback is caught by the next one.
    async fn wait_ai_ready(&mut self) {
        let chat = self.sentinel_chat;
        timeout(WAIT_TIMEOUT, async {
            loop {
                let ping = format!("{SENTINEL_MARKER}:ai:{}", self.next_sentinel_ping());
                self.queue_user(chat, &ping).await;
                let listener = self.listener.clone();
                let seen = timeout(Duration::from_millis(700), async {
                    loop {
                        if listener.count_for(SENTINEL_MARKER) >= 1 {
                            return;
                        }
                        sleep(Duration::from_millis(50)).await;
                    }
                })
                .await;
                if seen.is_ok() {
                    return;
                }
            }
        })
        .await
        .expect("ai_completions never reacted to the bootstrap sentinel");
    }

    /// sub-chat plugin readiness gate: a sentinel-chat state change must
    /// observably get its three tools registered — the per-chat registration
    /// callback is installed strictly before the `on_tool_call` subscription
    /// and the watcher hook in `run_plugin`, so this proves the whole
    /// live-handling chain.
    async fn wait_sub_chat_ready(&self) {
        let ping = format!("{SENTINEL_MARKER}:sub:{}", self.next_sentinel_ping());
        self.queue_user(self.sentinel_chat, &ping).await;
        self.wait_tools_registered(self.sentinel_chat).await;
    }

    fn next_sentinel_ping(&self) -> u32 {
        static PINGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        PINGS.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Spawn the sub-chat plugin fresh (also used by scenario 7 to model the
    /// restart whose startup pass runs Phase 6 recovery). Waits for
    /// registration visibility, a live ack of a probe event, AND the tool
    /// registration gate.
    pub async fn start_sub_chat_plugin(&mut self) {
        let url = self.server_url.clone();
        self.sub_plugin = Some(tokio::spawn(async move {
            rhd_plugin_sub_chat::plugin::run_plugin(&url, SUB_CHAT_PLUGIN_ID)
                .await
                .expect("sub_chat plugin task failed");
        }));
        self.wait_for_plugin_visible(SUB_CHAT_PLUGIN_ID).await;

        // Probe: emit an event and wait for the sub-chat plugin to ack it.
        // A registration that outran its event subscription would otherwise
        // stall the first real preRequest for 30 s (then error-tag the chat).
        let monitor = self
            .client
            .create_plugins_monitor()
            .await
            .expect("plugins monitor must start");
        ack_probe(self.client.as_ref(), &monitor).await;
        self.wait_sub_chat_ready().await;
    }

    async fn wait_for_plugin_visible(&self, plugin_id: &str) {
        let client = Arc::clone(&self.client);
        let visible_id = plugin_id.to_string();
        let check = timeout(WAIT_TIMEOUT, async move {
            loop {
                let plugins = client
                    .get_plugins(GetPluginsParams {})
                    .await
                    .expect("getPlugins failed")
                    .plugins;
                if plugins.iter().any(|p| p.plugin_id == visible_id) {
                    return;
                }
                sleep(Duration::from_millis(50)).await;
            }
        });
        check
            .await
            .unwrap_or_else(|_| panic!("plugin {plugin_id} never became visible"));
    }

    pub async fn plugin_client(&self) -> ChatClient {
        self.client.as_ref().clone()
    }

    pub async fn create_chat(&self, title: &str) -> i64 {
        self.client
            .create_chat(CreateChatParams {
                title: title.to_string(),
                tags: vec![],
            })
            .await
            .expect("createChat failed")
            .chat_id
    }

    pub async fn queue_user(&self, chat_id: i64, content: &str) {
        self.client
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
            .expect("addQueueMessage failed");
    }

    pub async fn chat(&self, chat_id: i64) -> rhd_chat_api::GetChatResult {
        super::get_chat(self.client.as_ref(), chat_id).await
    }

    pub async fn messages(&self, chat_id: i64) -> Vec<Message> {
        self.chat(chat_id).await.messages
    }

    pub async fn chat_tags(&self, chat_id: i64) -> Vec<String> {
        self.chat(chat_id).await.chat.tags
    }

    /// The subchat spawned for `tool_call_id` (found by its link tag), if any.
    pub async fn subchat_id(&self, tool_call_id: &str) -> Option<i64> {
        find_subchat(self.client.as_ref(), tool_call_id).await
    }

    /// Wait until the sub-chat plugin has registered its three tools on
    /// `chat_id`, so the first provider request demonstrably carried them.
    pub async fn wait_tools_registered(&self, chat_id: i64) {
        let client = Arc::clone(&self.client);
        self.wait_for("sub-chat tools registered", move || {
            let client = Arc::clone(&client);
            Box::pin(async move {
                let tools = client
                    .get_tools(GetToolsParams { chat_id })
                    .await
                    .expect("getTools failed")
                    .tools;
                let names: Vec<&str> =
                    tools.iter().map(|t| t.tool.function.name.as_str()).collect();
                ["rhd_sub_chat", "rhd_sub_chat_status", "rhd_sub_chat_await"]
                    .iter()
                    .all(|name| names.iter().any(|n| n == name))
            })
        })
        .await;
    }

    /// Plan-mandated convergence primitive: poll `check` with a generous
    /// deadline, never a fixed sleep. Panics naming `what` on timeout.
    pub async fn wait_for(&self, what: &str, mut check: impl FnMut() -> BoxFuture<'static, bool>) {
        timeout(WAIT_TIMEOUT, async {
            loop {
                if check().await {
                    return;
                }
                sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
    }

    pub async fn remove_tag(&self, chat_id: i64, tag: &str) {
        self.client
            .update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: vec![],
                remove_tags: vec![tag.to_string()],
            })
            .await
            .expect("updateChat(remove tag) failed");
    }

    pub async fn add_chat_tag(&self, chat_id: i64, tag: &str) {
        self.client
            .update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: vec![tag.to_string()],
                remove_tags: vec![],
            })
            .await
            .expect("updateChat(add tag) failed");
    }

    /// Abort the in-process plugin tasks (each test calls this at the end).
    pub fn shutdown(&self) {
        self._ai_plugin.abort();
        if let Some(handle) = self.sub_plugin.as_ref() {
            handle.abort();
        }
    }
}
