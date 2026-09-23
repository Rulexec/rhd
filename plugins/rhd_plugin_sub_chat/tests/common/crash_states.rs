//! Crash-state construction + recovery-pass driving for Phase 6 tests
//! (`tests/recovery_test.rs`). Chat-server-level fixtures only — the states
//! a plugin restart can find in stored history, assembled without
//! `ai_completions` or a real process kill.
//!
//! Lives in its own `common` submodule so the pre-existing Phase 5 harness
//! in `super` stays under the 400-line file limit; both compile into every
//! test binary (unused here is fine — `common/mod.rs` allows `dead_code`
//! crate-wide).

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, FunctionCall, GetQueueMessagesParams, ListChatsParams, ToolCall,
    UpdateMessageParams,
};
use rhd_chat_client::{ChatClient, ChatMonitor};
use rhd_plugin_sub_chat::recovery::{self, RecoveryReport};
use tokio::time::{sleep, timeout};

use super::{get_chat, TestEnv};

/// Synthetic `ToolCall` for stored unresolved-assistant-message building.
pub fn tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
        tags: vec![],
    }
}

/// Build a **stored** assistant message declaring unresolved tool calls —
/// exactly what `ai_completions` leaves behind when it dies mid-turn:
/// `addMessage(role=assistant)` then `updateMessage` with the `toolCalls`
/// JSON (the wire plan's Phase 6 note-1 construction). Verified against the
/// real server while writing these tests: `updateMessage` has no
/// role/permission gating (only existence), and the `Vec<rhd_chat_api::ToolCall>`
/// serde shape (an extra `"type"` key the DB's `rhd_db::ToolCall` ignores)
/// round-trips through storage into `getMessages` /
/// `withUnresolvedToolCalls`. `is_finished` is deliberately left `None` on
/// the update so no `assistantMessageWithToolCalls` broadcast fires — these
/// tests have no live `on_tool_call` subscription, keeping the scan the
/// sole driver.
pub async fn add_assistant_calls(client: &ChatClient, chat_id: i64, calls: &[ToolCall]) -> i64 {
    let message_id = client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: String::new(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .expect("addMessage(assistant) failed")
        .message_id;
    client
        .update_message(UpdateMessageParams {
            message_id,
            content: None,
            reasoning_content: None,
            role: None,
            add_tags: vec![],
            remove_tags: vec![],
            is_finished: None,
            is_streaming: None,
            tool_calls: Some(
                serde_json::to_string(calls).expect("tool calls must serialize"),
            ),
        })
        .await
        .expect("updateMessage(toolCalls) failed");
    message_id
}

/// Persisted `tool`-role answer for `tool_call_id` — the shape the
/// duplicate-answer guard finds (scenario 4's starting state).
pub async fn answer_call(client: &ChatClient, chat_id: i64, tool_call_id: &str, content: &str) {
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content: content.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .expect("addMessage(tool) failed");
}

/// The whole queue of `chat_id` as `(role, content)` pairs in order — the
/// seed/reconcile-order assertions.
pub async fn queue_contents(client: &ChatClient, chat_id: i64) -> Vec<(String, String)> {
    client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .expect("getQueueMessages failed")
        .messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone()))
        .collect()
}

/// Every chat that exists (the scan's `respawned`/no-double-create checks).
pub async fn chat_ids(client: &ChatClient) -> Vec<i64> {
    client
        .list_chats(ListChatsParams { tags: vec![] })
        .await
        .expect("listChats failed")
        .chats
        .iter()
        .map(|c| c.id)
        .collect()
}

/// Stored (non-queue) message count of `chat_id` — idempotency checks.
pub async fn message_count(client: &ChatClient, chat_id: i64) -> usize {
    get_chat(client, chat_id).await.messages.len()
}

/// Block until the monitor's tracked set includes `chat_id`. Two roles:
/// the scan iterates `get_chat_ids()`, so recovery must not run before the
/// `chatCreated` broadcast lands; and once a chat is tracked its per-chat
/// event subscription is registered (chat_monitor.rs inserts only after
/// subscribing), so later mutations do fire state-change events.
pub async fn wait_monitor_sees(monitor: &ChatMonitor, chat_id: i64) {
    timeout(Duration::from_secs(10), async {
        loop {
            if monitor.get_chat_ids().await.contains(&chat_id) {
                return;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("monitor never tracked chat {chat_id}"));
}

/// Drive one production recovery pass through the exact `plugin.rs` call
/// shape. The only fatal outcome is a failed index read — a harness bug,
/// so unwrap.
pub async fn run_recovery(env: &TestEnv) -> RecoveryReport {
    recovery::run(Arc::clone(&env.client), env.monitor.as_ref(), env.ctx.clone())
        .await
        .expect("recovery pass must not fail fatally")
}
