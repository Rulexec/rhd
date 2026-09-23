//! Integration tests: the spawn engine and the async answer path.
//!
//! Chat server + this plugin's code only — no ai_completions, no mock AI.
//! An assistant message with tool_calls cannot be created through
//! `addMessage` (no tool_calls param), so the tests drive the exported
//! `handler::handle_spawn` API directly (the crate is lib+bin; tests link the
//! lib) with a manually built `rhd_chat_api::ToolCall`.

use std::sync::Arc;

use rhd_chat_api::{
    ChatSummary, CreateChatParams, FunctionCall, GetChatParams, GetQueueMessagesParams,
    ListChatsParams, RegisterPluginParams, ToolCall,
};
use rhd_chat_client::ChatClient;
use rhd_plugin_sub_chat::{handler, reply, spawn, tags, watcher};

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

/// Connect the test client and register it as a plugin (production-shaped).
async fn connect_test_client(url: &str) -> Arc<ChatClient> {
    let client = Arc::new(
        ChatClient::connect(url)
            .await
            .expect("client connect failed"),
    );
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "test-spawn-client".to_string(),
        })
        .await
        .expect("plugin registration failed");
    client
}

/// Build a `HandlerCtx` with fresh guards and a watcher. These tests never
/// drive a subchat to completion, so the watcher only ever accumulates
/// parked waiters (sync calls stay unanswered here, as before Phase 4).
fn make_ctx(client: &Arc<ChatClient>) -> handler::HandlerCtx {
    let guards = Arc::new(reply::AnswerGuards::default());
    let watcher = Arc::new(watcher::Watcher::new(
        Arc::clone(client),
        Arc::clone(&guards),
    ));
    handler::HandlerCtx {
        client: Arc::clone(client),
        watcher,
        guards,
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

/// Chats carrying the `sub_chat:call:<tool_call_id>` link tag.
async fn link_tagged_chats(client: &ChatClient, tool_call_id: &str) -> Vec<ChatSummary> {
    let link = tags::call_tag(tool_call_id);
    client
        .list_chats(ListChatsParams {
            tags: vec![link.clone()],
        })
        .await
        .expect("listChats failed")
        .chats
        .into_iter()
        .filter(|c| c.tags.iter().any(|t| t == &link))
        .collect()
}

fn tool_answers(
    result: &rhd_chat_api::GetChatResult,
    tool_call_id: &str,
) -> Vec<rhd_chat_api::Message> {
    result
        .messages
        .iter()
        .filter(|m| m.tool_call_id.as_deref() == Some(tool_call_id))
        .cloned()
        .collect()
}

/// Async spawn: one subchat with the full Wire-Contract tag set (no `paused`
/// left), starting messages queued in order, caller self-tagged as root, and
/// the exact background notice as the tool answer.
#[tokio::test]
async fn async_spawn_creates_activated_subchat_and_answers_notice() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = connect_test_client(&url).await;

    let chat_a = client
        .create_chat(CreateChatParams {
            title: "caller".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat A failed")
        .chat_id;

    let arguments = r#"{"messages":[{"role":"system","content":"You are a focused worker."},{"role":"user","content":"Count to three"}],"tags":["mcp:common"],"async":true}"#;
    let ctx = make_ctx(&client);
    handler::handle_spawn(ctx, chat_a, spawn_tool_call("call_async_1", arguments)).await;

    // Exactly one subchat carries the link tag.
    let subchats = link_tagged_chats(&client, "call_async_1").await;
    assert_eq!(subchats.len(), 1, "expected exactly one linked subchat");
    let sub = &subchats[0];

    // Wire-Contract tag set, `paused` removed at activation.
    assert!(sub.tags.contains(&tags::parent_tag(chat_a)));
    assert!(sub.tags.contains(&tags::root_tag(chat_a)));
    assert!(sub.tags.contains(&tags::call_tag("call_async_1")));
    assert!(sub.tags.contains(&"mcp:common".to_string()));
    assert!(
        !sub.tags.iter().any(|t| t == tags::PAUSED_TAG),
        "paused must be removed on activation, got {:?}",
        sub.tags
    );

    // Generated title from the first user message.
    assert_eq!(sub.title, "sub · Count to three");

    // Caller A carries its own root tag (first spawner).
    let a = client
        .get_chat(GetChatParams {
            chat_id: chat_a,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat A failed");
    assert!(a.chat.tags.contains(&tags::root_tag(chat_a)));

    // Starting messages sit in the queue in seed order (system first).
    let queue = client
        .get_queue_messages(GetQueueMessagesParams { chat_id: sub.id })
        .await
        .expect("getQueueMessages failed")
        .messages;
    assert_eq!(queue.len(), 2, "both starting messages must be queued");
    assert_eq!(queue[0].role, "system");
    assert_eq!(queue[0].content, "You are a focused worker.");
    assert_eq!(queue[1].role, "user");
    assert_eq!(queue[1].content, "Count to three");

    // A holds exactly one tool answer: the exact async notice for B.
    let answers = tool_answers(&a, "call_async_1");
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].role, "tool");
    assert_eq!(answers[0].content, spawn::async_notice(sub.id));
}

/// Idempotency: a full second run with the same tool call creates no second
/// chat, posts no duplicate answer, and leaves the queue untouched.
#[tokio::test]
async fn async_spawn_rerun_is_idempotent() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = connect_test_client(&url).await;

    let chat_a = client
        .create_chat(CreateChatParams {
            title: "caller".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat A failed")
        .chat_id;

    let arguments = r#"{"messages":[{"role":"user","content":"Repeated spawn"}],"async":true}"#;
    handler::handle_spawn(
        make_ctx(&client),
        chat_a,
        spawn_tool_call("call_dup", arguments),
    )
    .await;
    let first = link_tagged_chats(&client, "call_dup").await;
    assert_eq!(first.len(), 1);
    let sub_id = first[0].id;

    // Second run: the has_tool_result guard short-circuits before converge.
    handler::handle_spawn(
        make_ctx(&client),
        chat_a,
        spawn_tool_call("call_dup", arguments),
    )
    .await;

    assert_eq!(
        link_tagged_chats(&client, "call_dup").await.len(),
        1,
        "no second chat may be created"
    );

    let a = client
        .get_chat(GetChatParams {
            chat_id: chat_a,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat A failed");
    assert_eq!(
        tool_answers(&a, "call_dup").len(),
        1,
        "no duplicate tool answer"
    );

    let queue = client
        .get_queue_messages(GetQueueMessagesParams { chat_id: sub_id })
        .await
        .expect("getQueueMessages failed")
        .messages;
    assert_eq!(queue.len(), 1, "queue must be unchanged");
    assert_eq!(queue[0].content, "Repeated spawn");
}

/// Sync spawn converges identically but leaves the call unanswered (Phase 4
/// seam). A re-run before any answer exercises converge_spawn's link-tag
/// reuse: no second chat, no duplicate queue items, still parked.
#[tokio::test]
async fn sync_spawn_parks_call_and_reuses_subchat_on_rerun() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = connect_test_client(&url).await;

    let chat_a = client
        .create_chat(CreateChatParams {
            title: "caller".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat A failed")
        .chat_id;

    let arguments =
        r#"{"messages":[{"role":"system","content":"ctx"},{"role":"user","content":"sync task"}]}"#;
    handler::handle_spawn(
        make_ctx(&client),
        chat_a,
        spawn_tool_call("call_sync_1", arguments),
    )
    .await;

    let subchats = link_tagged_chats(&client, "call_sync_1").await;
    assert_eq!(subchats.len(), 1);
    let sub_id = subchats[0].id;
    assert!(
        !subchats[0].tags.iter().any(|t| t == tags::PAUSED_TAG),
        "sync spawn must activate the subchat too"
    );

    // The seam: the call stays unanswered (parked parent loop).
    let a = client
        .get_chat(GetChatParams {
            chat_id: chat_a,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat A failed");
    assert!(tool_answers(&a, "call_sync_1").is_empty());

    // Re-run while unanswered: converge finds the existing chat by link tag,
    // sees no `paused`, and must not re-enqueue or duplicate anything.
    handler::handle_spawn(
        make_ctx(&client),
        chat_a,
        spawn_tool_call("call_sync_1", arguments),
    )
    .await;

    assert_eq!(link_tagged_chats(&client, "call_sync_1").await.len(), 1);
    let queue = client
        .get_queue_messages(GetQueueMessagesParams { chat_id: sub_id })
        .await
        .expect("getQueueMessages failed")
        .messages;
    assert_eq!(queue.len(), 2, "queue must not be re-populated");
}

/// Validation failure: single-line `rhd_sub_chat error: ` answer in A and no
/// chat created (AD-13).
#[tokio::test]
async fn validation_error_answers_and_creates_nothing() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{port}/");
    let client = connect_test_client(&url).await;

    let chat_a = client
        .create_chat(CreateChatParams {
            title: "caller".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat A failed")
        .chat_id;

    let arguments = r#"{"messages":[{"role":"assistant","content":"not allowed"}],"async":true}"#;
    handler::handle_spawn(
        make_ctx(&client),
        chat_a,
        spawn_tool_call("call_bad", arguments),
    )
    .await;

    // No subchat for the rejected call, and A was not even root-tagged.
    assert!(link_tagged_chats(&client, "call_bad").await.is_empty());

    let a = client
        .get_chat(GetChatParams {
            chat_id: chat_a,
            if_version_higher_than: None,
        })
        .await
        .expect("getChat A failed");
    assert!(
        !a.chat.tags.contains(&tags::root_tag(chat_a)),
        "validation must fail before any state change"
    );

    let answers = tool_answers(&a, "call_bad");
    assert_eq!(answers.len(), 1, "exactly one error answer");
    assert_eq!(answers[0].role, "tool");
    assert!(
        answers[0].content.starts_with("rhd_sub_chat error: "),
        "unexpected answer: {:?}",
        answers[0].content
    );
    assert!(
        !answers[0].content.contains('\n'),
        "answer must be single-line"
    );
}
