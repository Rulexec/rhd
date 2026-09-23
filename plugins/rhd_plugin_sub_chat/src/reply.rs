//! Idempotent tool-call answering helpers.
//!
//! Every phase of this plugin answers (and guards answers to) tool calls
//! through this module, so the duplicate-answer discipline lives in exactly
//! one place.

use std::collections::HashSet;

use rhd_chat_api::{AddMessageParams, GetChatParams};
use rhd_chat_client::{ChatClient, ClientError};

/// True if a `tool`-role message answering `tool_call_id` already exists in
/// `chat_id` (todo_list's has_tool_result pattern; guard against duplicate answers).
pub async fn has_tool_result(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
) -> Result<bool, ClientError> {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await?;
    Ok(chat
        .messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some(tool_call_id)))
}

/// Post a `tool`-role answer for `tool_call_id` in `chat_id`.
pub async fn answer_tool_call(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
    content: String,
) -> Result<(), ClientError> {
    // The client returns the new message id; callers only care about success.
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content,
            tool_call_id: Some(tool_call_id.to_string()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .map(|_result| ())
}

/// In-flight/delivered `tool_call_id` dedup across the concurrent answer paths
/// (monitor hook vs `Watcher::register_or_complete` vs the Phase 6 recovery
/// scan). The persisted [`has_tool_result`] check alone races: two paths can
/// both observe "no result yet" before either posts one, so the in-memory set
/// closes the window, and removing the id again on any failure keeps retries
/// possible after a transient error.
#[derive(Default)]
pub struct AnswerGuards {
    answered: tokio::sync::Mutex<HashSet<String>>,
}

impl AnswerGuards {
    /// Check the in-memory set + the persisted history, mark, answer, unmark
    /// on error.
    ///
    /// Returns `Ok(true)` when the answer was sent, `Ok(false)` when skipped as
    /// a duplicate (in flight or already present in the chat history). An
    /// `Err` propagates from either the history check or the post itself with
    /// the in-flight mark removed, so a later path may retry the call.
    pub async fn answer_once(
        &self,
        client: &ChatClient,
        chat_id: i64,
        tool_call_id: &str,
        content: String,
    ) -> Result<bool, ClientError> {
        // 1. Lock → dup-check → mark → drop the lock (the set is held only
        // across the check, never across the network calls).
        {
            let mut answered = self.answered.lock().await;
            if answered.contains(tool_call_id) {
                return Ok(false);
            }
            answered.insert(tool_call_id.to_string());
        }

        // 2. Persisted guard: a result already in the chat → unmark, skip.
        // Any failure unmarks too, so a later event or recovery pass can retry.
        let outcome = match has_tool_result(client, chat_id, tool_call_id).await {
            Ok(true) => Ok(false),
            Ok(false) => answer_tool_call(client, chat_id, tool_call_id, content)
                .await
                .map(|()| true),
            Err(e) => Err(e),
        };
        if matches!(outcome, Err(_) | Ok(false)) {
            self.answered.lock().await.remove(tool_call_id);
        }
        outcome
    }
}
