//! Shared harness for the sub-chat status/await integration tests (Phase 5).
//!
//! Each integration-test binary compiles its own copy of this module, so
//! helpers that are unused in a given binary are expected — hence the
//! crate-wide `dead_code` allowance below (same pattern as
//! `plugins/rhd_plugin_mcp/tests/common/mod.rs`).
//!
//! Production-shaped: chat server + this plugin's code only — no
//! ai_completions, no mock AI. Subchats are spawned via
//! `handler::handle_spawn` (async mode: the spawn itself answers, leaving the
//! watcher pristine for await assertions); status/await calls are driven
//! through the production dispatch entry `handler::handle_tool_call_event`
//! with synthetic events, exactly as the `on_tool_call` subscription would
//! deliver them. Phase 6 crash-state fixtures live in the [`crash_states`]
//! submodule; the Phase 7 full-pipeline harness (chat server + ai_completions
//! + this plugin + content-routed mock AI) lives in [`routing`] (the
//! `RoutingListener`), [`bootstrap`] (server/config/probe plumbing),
//! [`full_env`] (the `FullEnv` itself) and [`polling`] (its wait helpers).

#![allow(dead_code)]

pub mod bootstrap;
pub mod crash_states;
pub mod full_env;
pub mod polling;
pub mod routing;

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, AddQueueMessageParams, AssistantMessageWithToolCallsData, ChatSummary,
    CreateChatParams, DeleteQueueMessageParams, FunctionCall, GetChatParams,
    GetQueueMessagesParams, ListChatsParams, Message, RegisterPluginParams, ToolCall,
};
use rhd_chat_client::{ChatClient, ChatMonitor};
use rhd_plugin_sub_chat::{
    handler::{self, HandlerCtx},
    reply::AnswerGuards,
    tags,
    watcher::{self, Watcher},
};
use tokio::time::{sleep, timeout};

pub struct TestEnv {
    pub client: Arc<ChatClient>,
    pub ctx: HandlerCtx,
    pub watcher: Arc<Watcher>,
    pub monitor: Arc<ChatMonitor>,
}

/// Start an ephemeral-port, in-memory chat server.
async fn start_test_server() -> u16 {
    let chat_config = rhd_chat_server::config::Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        db_path: ":memory:".to_string(),
        clear_pending_acks: false,
    };
    let (port, _server_handle) = rhd_chat_server::server::start(chat_config)
        .await
        .expect("Failed to start chat server");
    port
}

/// Production-shaped harness: client + monitor on all chats + watcher hook
/// installed (mirrors `plugin::run_plugin`, as in sync_spawn_test.rs).
pub async fn setup() -> TestEnv {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = Arc::new(
        ChatClient::connect(&url)
            .await
            .expect("client connect failed"),
    );
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "test-status-await".to_string(),
        })
        .await
        .expect("plugin registration failed");

    let monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .expect("create monitor failed"),
    );
    monitor
        .subscribe_to_all_chats()
        .await
        .expect("subscribe failed");

    let guards = Arc::new(AnswerGuards::default());
    let watcher = Arc::new(Watcher::new(Arc::clone(&client), Arc::clone(&guards)));
    watcher::install(&monitor, Arc::clone(&watcher)).await;

    let ctx = HandlerCtx {
        client: Arc::clone(&client),
        watcher: Arc::clone(&watcher),
        guards,
    };
    TestEnv {
        client,
        ctx,
        watcher,
        monitor,
    }
}

pub async fn create_chat(client: &ChatClient, title: &str, tags: Vec<String>) -> i64 {
    client
        .create_chat(CreateChatParams {
            title: title.to_string(),
            tags,
        })
        .await
        .expect("create chat failed")
        .chat_id
}

pub async fn get_chat(client: &ChatClient, chat_id: i64) -> rhd_chat_api::GetChatResult {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat failed")
}

/// Chat tags minus the orthogonal `ai_completions:*` lifecycle tags, sorted —
/// the exact sub-chat lineage tag-set assertions (Phase 7 e2e). The lifecycle
/// tags appear/disappear on their plugin's own schedule, so "exact set" is
/// always relative to them.
pub fn production_tags(tags: &[String]) -> Vec<String> {
    let mut filtered: Vec<String> = tags
        .iter()
        .filter(|tag| !tag.starts_with("ai_completions:"))
        .cloned()
        .collect();
    filtered.sort();
    filtered
}

/// The chat's last message when it is a finished, non-streaming assistant
/// message without tool calls — the completion-predicate shape, read live.
pub fn final_assistant_message(messages: &[Message]) -> Option<Message> {
    messages
        .last()
        .filter(|m| m.role == "assistant" && m.is_finished && !m.is_streaming && m.tool_calls.is_empty())
        .cloned()
}

/// True when the chat has a finished assistant message declaring `tool_call_id`.
pub fn history_has_assistant_call(messages: &[Message], tool_call_id: &str) -> bool {
    messages.iter().any(|m| {
        m.role == "assistant"
            && m.is_finished
            && m.tool_calls.iter().any(|c| c.id == tool_call_id)
    })
}

/// `tool`-role messages in `chat_id` answering `tool_call_id`.
pub async fn tool_answers(client: &ChatClient, chat_id: i64, tool_call_id: &str) -> Vec<Message> {
    get_chat(client, chat_id)
        .await
        .messages
        .iter()
        .filter(|m| m.tool_call_id.as_deref() == Some(tool_call_id))
        .cloned()
        .collect()
}

/// Exactly one `tool` answer for `tool_call_id`, byte-identical to `expected`.
pub async fn assert_answer(client: &ChatClient, chat_id: i64, tool_call_id: &str, expected: &str) {
    let answers = tool_answers(client, chat_id, tool_call_id).await;
    assert_eq!(answers.len(), 1, "exactly one answer for {tool_call_id}");
    assert_eq!(answers[0].role, "tool");
    assert_eq!(answers[0].content, expected);
}

/// Poll until every id in `tool_call_ids` has at least one answer in `chat_id`.
pub async fn wait_for_answers(client: &ChatClient, chat_id: i64, tool_call_ids: &[&str]) {
    timeout(Duration::from_secs(10), async {
        loop {
            let chat = get_chat(client, chat_id).await;
            let all_answered = tool_call_ids.iter().all(|id| {
                chat.messages
                    .iter()
                    .any(|m| m.tool_call_id.as_deref() == Some(*id))
            });
            if all_answered {
                return;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("chat {chat_id} never got answers for {tool_call_ids:?}"));
}

/// Let any spawned drain/answer tasks run their course before a negative
/// assertion.
pub async fn settle() {
    sleep(Duration::from_millis(500)).await;
}

/// Drive one tool call through the production `on_tool_call` dispatch entry.
pub async fn drive_tool_call(
    env: &TestEnv,
    caller_chat_id: i64,
    name: &str,
    tool_call_id: &str,
    arguments: &str,
) {
    let message = Message {
        id: 0,
        chat_id: caller_chat_id,
        role: "assistant".to_string(),
        content: String::new(),
        tool_call_id: None,
        created_at: "2026-09-24T00:00:00Z".parse().unwrap(),
        reasoning_content: None,
        tags: vec![],
        is_finished: true,
        is_streaming: false,
        tool_calls: vec![ToolCall {
            id: tool_call_id.to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
            tags: vec![],
        }],
    };
    handler::handle_tool_call_event(
        env.ctx.clone(),
        AssistantMessageWithToolCallsData {
            chat_id: caller_chat_id,
            message,
            chat_version: 0,
            tool_names: vec![name.to_string()],
        },
    )
    .await;
}

/// Async spawn from `caller_chat_id` via `handle_spawn`; returns the
/// activated subchat id (callers stay free of parked spawn waiters).
pub async fn async_spawn(env: &TestEnv, caller_chat_id: i64, tool_call_id: &str, task: &str) -> i64 {
    let arguments = format!(r#"{{"messages":[{{"role":"user","content":"{task}"}}],"async":true}}"#);
    handler::handle_spawn(
        env.ctx.clone(),
        caller_chat_id,
        ToolCall {
            id: tool_call_id.to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "rhd_sub_chat".to_string(),
                arguments,
            },
            tags: vec![],
        },
    )
    .await;
    let link = tags::call_tag(tool_call_id);
    let subchats: Vec<ChatSummary> = env
        .client
        .list_chats(ListChatsParams {
            tags: vec![link.clone()],
        })
        .await
        .expect("listChats failed")
        .chats
        .into_iter()
        .filter(|c| c.tags.iter().any(|t| t == &link))
        .collect();
    assert_eq!(subchats.len(), 1, "expected exactly one linked subchat");
    subchats[0].id
}

/// Delete every queued message of `chat_id` (production: `ai_completions`
/// drains the queue; the predicate requires `queued_messages_count == 0`).
pub async fn drain_queue(client: &ChatClient, chat_id: i64) {
    let queue = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .expect("getQueueMessages failed")
        .messages;
    for message in queue {
        client
            .delete_queue_message(DeleteQueueMessageParams {
                message_id: message.id,
            })
            .await
            .expect("deleteQueueMessage failed");
    }
}

pub async fn add_finished_message(client: &ChatClient, chat_id: i64, role: &str, content: &str) {
    client
        .add_message(AddMessageParams {
            chat_id,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .expect("addMessage failed");
}

pub async fn add_queued_message(client: &ChatClient, chat_id: i64, content: &str) {
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
        .expect("addQueueMessage failed");
}
