//! Integration tests: the Phase 4 sync answer path — completion watcher.
//!
//! Chat server + this plugin's code only — no ai_completions, no mock AI.
//! Subchat completion is driven synthetically: spawn via `handle_spawn`
//! (async=false), clear the seed queue (mirroring what `ai_completions`
//! drains in production), then append messages so the `ChatMonitor` hook
//! evaluates the predicate on real state-change events.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, ChatSummary, CreateChatParams, DeleteQueueMessageParams, FunctionCall,
    GetChatParams, GetQueueMessagesParams, ListChatsParams, Message, RegisterPluginParams,
    ToolCall, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, ChatMonitor, ChatState};
use rhd_plugin_sub_chat::{
    handler::{self, HandlerCtx},
    reply::AnswerGuards,
    tags,
    watcher::{self, Waiter, Watcher},
};
use tokio::time::{sleep, timeout};

struct TestEnv {
    client: Arc<ChatClient>,
    ctx: HandlerCtx,
    watcher: Arc<Watcher>,
    monitor: Arc<ChatMonitor>,
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

/// Production-shaped harness: client + monitor subscribed to all chats +
/// the watcher hook installed (exactly what `plugin::run_plugin` wires).
async fn setup() -> TestEnv {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = Arc::new(
        ChatClient::connect(&url)
            .await
            .expect("client connect failed"),
    );
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "test-sync-client".to_string(),
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

fn spawn_tool_call(tool_call_id: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: tool_call_id.to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "rhd_sub_chat".to_string(),
            arguments: arguments.to_string(),
        },
        tags: vec![],
    }
}

async fn create_chat(client: &ChatClient, title: &str) -> i64 {
    client
        .create_chat(CreateChatParams {
            title: title.to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat failed")
        .chat_id
}

async fn get_chat(client: &ChatClient, chat_id: i64) -> rhd_chat_api::GetChatResult {
    client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat failed")
}

/// `tool`-role messages in `chat_id` answering `tool_call_id`.
async fn tool_answers(client: &ChatClient, chat_id: i64, tool_call_id: &str) -> Vec<Message> {
    get_chat(client, chat_id)
        .await
        .messages
        .iter()
        .filter(|m| m.tool_call_id.as_deref() == Some(tool_call_id))
        .cloned()
        .collect()
}

/// Poll until every id in `tool_call_ids` has at least one answer in `chat_id`.
async fn wait_for_answers(client: &ChatClient, chat_id: i64, tool_call_ids: &[&str]) {
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

/// Wait until the monitor's own view of `chat_id` satisfies `pred` — proves
/// the state-change hook has fired (and its task spawned) for that state.
async fn wait_monitor_state(
    monitor: &ChatMonitor,
    chat_id: i64,
    label: &str,
    pred: impl Fn(&ChatState) -> bool,
) {
    timeout(Duration::from_secs(10), async {
        loop {
            if let Some(state) = monitor.get_chat_state(chat_id).await {
                if pred(&state) {
                    return;
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("monitor never saw {label} for chat {chat_id}"));
}

/// Let any spawned drain/answer tasks run their course before a negative
/// assertion.
async fn settle() {
    sleep(Duration::from_millis(500)).await;
}

/// Drive a sync `rhd_sub_chat` spawn and return the activated subchat id.
async fn sync_spawn(env: &TestEnv, caller_chat_id: i64, tool_call_id: &str, task: &str) -> i64 {
    let arguments = format!(
        r#"{{"messages":[{{"role":"system","content":"ctx"}},{{"role":"user","content":"{task}"}}]}}"#
    );
    handler::handle_spawn(
        env.ctx.clone(),
        caller_chat_id,
        spawn_tool_call(tool_call_id, &arguments),
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
    assert!(
        !subchats[0].tags.iter().any(|t| t == tags::PAUSED_TAG),
        "spawn must activate the subchat"
    );
    subchats[0].id
}

/// Delete every queued message of `chat_id` (production: `ai_completions`
/// drains the queue; the predicate requires `queued_messages_count == 0`).
async fn drain_queue(client: &ChatClient, chat_id: i64) {
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

async fn add_finished_message(client: &ChatClient, chat_id: i64, role: &str, content: &str) {
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

async fn toggle_tags(client: &ChatClient, chat_id: i64, tag: &str) {
    client
        .update_chat(UpdateChatParams {
            chat_id,
            title: None,
            add_tags: vec![tag.to_string()],
            remove_tags: vec![],
        })
        .await
        .expect("add tag failed");
    client
        .update_chat(UpdateChatParams {
            chat_id,
            title: None,
            add_tags: vec![],
            remove_tags: vec![tag.to_string()],
        })
        .await
        .expect("remove tag failed");
}

/// Plan scenarios 1–3: the sync call parks after spawn; intermediate states
/// (no messages, last=user) keep it unanswered; the first finished assistant
/// message answers it byte-identically and consumes the waiter.
#[tokio::test]
async fn sync_spawn_parks_then_completion_answers_verbatim() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller").await;

    // Park: after converge + register, A's call must stay unanswered.
    let chat_b = sync_spawn(&env, chat_a, "call_sync_1", "sync task").await;
    assert!(
        env.watcher.watched_ids().await.contains(&chat_b),
        "the sync call must register a waiter on B"
    );
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "scenario 1: sync call stays unanswered right after spawn"
    );

    // Negative state 1 (plan step 2's precondition): queue drained, but no
    // messages at all → predicate false.
    drain_queue(&env.client, chat_b).await;
    wait_monitor_state(&env.monitor, chat_b, "empty queue", |s| {
        s.queued_messages_count == 0 && s.messages.is_empty()
    })
    .await;
    settle().await;
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "empty history must not complete the subchat"
    );

    // Negative state 2 (plan scenario 3): last message is user → predicate false.
    add_finished_message(&env.client, chat_b, "user", "intermediate").await;
    wait_monitor_state(&env.monitor, chat_b, "user message last", |s| {
        s.messages.last().is_some_and(|m| m.role == "user")
    })
    .await;
    settle().await;
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "a trailing user message must not complete the subchat"
    );

    // Plan scenario 2: finished assistant message → hook fires → A answered
    // with the content byte-identical.
    add_finished_message(&env.client, chat_b, "assistant", "SUBCHAT FINAL ANSWER").await;
    wait_for_answers(&env.client, chat_a, &["call_sync_1"]).await;
    let answers = tool_answers(&env.client, chat_a, "call_sync_1").await;
    assert_eq!(answers.len(), 1, "exactly one answer");
    assert_eq!(answers[0].role, "tool");
    assert_eq!(answers[0].content, "SUBCHAT FINAL ANSWER");
    assert!(
        !env.watcher.watched_ids().await.contains(&chat_b),
        "waiters must be consumed by the drain (take-before-answer)"
    );
}

/// Plan scenario 4: a burst of completion-bearing state-change events (the
/// final message plus two tag add/remove cycles) yields exactly one answer —
/// atomic take + AnswerGuards kill the duplicates.
#[tokio::test]
async fn duplicate_completion_events_answer_exactly_once() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller").await;
    let chat_b = sync_spawn(&env, chat_a, "call_dup_events", "dup task").await;
    assert!(
        tool_answers(&env.client, chat_a, "call_dup_events")
            .await
            .is_empty(),
        "still parked after spawn"
    );
    drain_queue(&env.client, chat_b).await;

    // Complete B, then hammer it with tag toggles: every event carries a
    // completed state, all drain attempts race on the atomic take.
    add_finished_message(&env.client, chat_b, "assistant", "DUP FINAL ANSWER").await;
    toggle_tags(&env.client, chat_b, "watcher:toggle1").await;
    toggle_tags(&env.client, chat_b, "watcher:toggle2").await;

    wait_for_answers(&env.client, chat_a, &["call_dup_events"]).await;
    settle().await; // let any straggler drain run before counting
    let answers = tool_answers(&env.client, chat_a, "call_dup_events").await;
    assert_eq!(
        answers.len(),
        1,
        "duplicate completion events must produce exactly one answer, got {:?}",
        answers.iter().map(|a| &a.content).collect::<Vec<_>>()
    );
    assert_eq!(answers[0].content, "DUP FINAL ANSWER");
}

/// Plan scenario 5 (+ impl note 1): several waiters on one subchat all get
/// the same content from a single completion; a waiter registered after the
/// completion is answered immediately by `register_or_complete` itself
/// (the register-then-check race closer).
#[tokio::test]
async fn multiple_waiters_broadcast_and_late_registration_answers_immediately() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller").await;
    let chat_b = sync_spawn(&env, chat_a, "call_w1", "broadcast task").await;
    drain_queue(&env.client, chat_b).await;

    // Second waiter on the same, still-incomplete subchat (simulates a
    // concurrent sync spawn / a Phase 5 `rhd_sub_chat_await`).
    env.watcher
        .register_or_complete(
            chat_b,
            Waiter {
                parent_chat_id: chat_a,
                tool_call_id: "call_w2".to_string(),
            },
        )
        .await;
    assert!(
        tool_answers(&env.client, chat_a, "call_w2")
            .await
            .is_empty(),
        "registering on an incomplete subchat must not answer"
    );

    // Completion broadcasts to both waiters with identical content.
    add_finished_message(&env.client, chat_b, "assistant", "SHARED ANSWER").await;
    wait_for_answers(&env.client, chat_a, &["call_w1", "call_w2"]).await;
    settle().await;
    let first = tool_answers(&env.client, chat_a, "call_w1").await;
    let second = tool_answers(&env.client, chat_a, "call_w2").await;
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].content, "SHARED ANSWER");
    assert_eq!(
        first[0].content, second[0].content,
        "both waiters get the same content"
    );

    // Late registration on an already-completed subchat: answered
    // immediately by register_or_complete's own check, with no new event on
    // B ever firing.
    env.watcher
        .register_or_complete(
            chat_b,
            Waiter {
                parent_chat_id: chat_a,
                tool_call_id: "call_w3".to_string(),
            },
        )
        .await;
    wait_for_answers(&env.client, chat_a, &["call_w3"]).await;
    let late = tool_answers(&env.client, chat_a, "call_w3").await;
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].content, "SHARED ANSWER");
    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "all waiters consumed"
    );
}
