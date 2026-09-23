//! Sync-spawn driving + completion-watcher fixtures for the Phase 4 tests
//! (`tests/sync_spawn_test.rs`, `tests/sync_spawn_broadcast_test.rs`).
//!
//! Lives in its own `common` submodule so the pre-existing Phase 5 harness
//! in `super` stays under the 400-line file limit (same pattern as
//! `crash_states`); every helper here was a `sync_spawn_test.rs` local
//! fixture before the Phase 4 file was split by scenario. All test binaries
//! compile the module — unused is fine, `common/mod.rs` allows `dead_code`
//! crate-wide.

use std::time::Duration;

use rhd_chat_api::{ChatSummary, FunctionCall, ListChatsParams, ToolCall, UpdateChatParams};
use rhd_chat_client::{ChatClient, ChatMonitor, ChatState};
use rhd_plugin_sub_chat::{handler, tags};
use tokio::time::{sleep, timeout};

use super::TestEnv;

/// Drive a sync `rhd_sub_chat` spawn and return the activated subchat id.
pub async fn sync_spawn(
    env: &TestEnv,
    caller_chat_id: i64,
    tool_call_id: &str,
    task: &str,
) -> i64 {
    let arguments = format!(
        r#"{{"messages":[{{"role":"system","content":"ctx"}},{{"role":"user","content":"{task}"}}]}}"#
    );
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
    assert!(
        !subchats[0].tags.iter().any(|t| t == tags::PAUSED_TAG),
        "spawn must activate the subchat"
    );
    subchats[0].id
}

/// Wait until the monitor's own view of `chat_id` satisfies `pred` — proves
/// the state-change hook has fired (and its task spawned) for that state.
pub async fn wait_monitor_state(
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

/// Add `tag` to `chat_id`, then remove it again — one round-trip fires two
/// extra state-change events whose states still satisfy the completion
/// predicate; the duplicate-completion burst driver.
pub async fn toggle_tags(client: &ChatClient, chat_id: i64, tag: &str) {
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
