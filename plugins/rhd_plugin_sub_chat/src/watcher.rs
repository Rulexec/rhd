//! Completion watcher: in-memory registry of tool-call waiters keyed by
//! subchat id, fanned by a single `ChatMonitor` state-change hook (AD-1/AD-2).
//!
//! A waiter is "answer `tool_call_id` in `parent_chat_id` when subchat X
//! satisfies [`completion::is_completed`]". The map is never persisted — the
//! Phase 6 recovery rebuilds it from unresolved tool calls (AD-7).

use std::collections::HashMap;
use std::sync::Arc;

use rhd_chat_api::{GetChatParams, GetChatResult};
use rhd_chat_client::{ChatClient, ChatMonitor, ChatState};
use tokio::sync::RwLock;

use crate::completion;
use crate::reply::AnswerGuards;

/// One parked tool call awaiting its subchat's final answer.
#[derive(Debug, Clone)]
pub struct Waiter {
    pub parent_chat_id: i64,
    pub tool_call_id: String,
}

/// Registry of waiters plus the answer machinery. Cheap to share
/// (`Arc<Watcher>` everywhere).
pub struct Watcher {
    client: Arc<ChatClient>,
    guards: Arc<AnswerGuards>,
    waiters: RwLock<HashMap<i64, Vec<Waiter>>>,
}

impl Watcher {
    pub fn new(client: Arc<ChatClient>, guards: Arc<AnswerGuards>) -> Self {
        Self {
            client,
            guards,
            waiters: RwLock::new(HashMap::new()),
        }
    }

    /// Register `waiter` for subchat `sub_chat_id`, then immediately evaluate
    /// the current state: the subchat may **already** be completed (fast mock
    /// answers, human-seeded history, or the completion state-change landed
    /// while this call was still converging), and no further event would ever
    /// wake the waiter otherwise. Insert-before-check is what closes that
    /// race — check-before-insert could leave a permanently parked call.
    /// Answers go through the same guarded `drain_and_answer` as the hook.
    pub async fn register_or_complete(&self, sub_chat_id: i64, waiter: Waiter) {
        self.waiters
            .write()
            .await
            .entry(sub_chat_id)
            .or_default()
            .push(waiter);

        // A failed state fetch leaves the waiter registered: if the chat
        // exists at all, its next state change evaluates the predicate; a
        // deleted subchat simply keeps the parent parked (no-timeouts policy).
        let result = match self
            .client
            .get_chat(GetChatParams {
                chat_id: sub_chat_id,
                if_version_higher_than: None,
            })
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!(
                    sub_chat_id,
                    error = %e,
                    "watcher: failed to fetch subchat state after registering waiter; waiting for the next state change"
                );
                return;
            }
        };
        let state = chat_state_from_get(&result);
        if completion::is_completed(&state) {
            self.drain_and_answer(sub_chat_id, &state).await;
        }
    }

    /// Atomic take + broadcast answer. The whole waiter list for `sub_chat_id`
    /// is removed under one write lock **before** any answer is posted, so
    /// concurrent state-change events (and `register_or_complete`) can never
    /// double-fire: the second taker sees an empty slot. Failed answers are
    /// logged and dropped — a deleted parent is the user's problem (AD-7) —
    /// never re-queued.
    pub(crate) async fn drain_and_answer(&self, sub_chat_id: i64, state: &ChatState) {
        let Some(waiters) = self.waiters.write().await.remove(&sub_chat_id) else {
            return; // someone else already took (or nothing registered yet)
        };
        let content = completion::final_answer(state)
            .unwrap_or_default()
            .to_string();
        for waiter in waiters {
            match self
                .guards
                .answer_once(
                    &self.client,
                    waiter.parent_chat_id,
                    &waiter.tool_call_id,
                    content.clone(),
                )
                .await
            {
                Ok(true) => tracing::info!(
                    sub_chat_id,
                    parent_chat_id = waiter.parent_chat_id,
                    tool_call_id = %waiter.tool_call_id,
                    "answered parked sub-chat tool call"
                ),
                // Duplicate in flight / already persisted — correct silence.
                Ok(false) => tracing::debug!(
                    sub_chat_id,
                    parent_chat_id = waiter.parent_chat_id,
                    tool_call_id = %waiter.tool_call_id,
                    "skipped duplicate answer"
                ),
                Err(e) => tracing::error!(
                    sub_chat_id,
                    parent_chat_id = waiter.parent_chat_id,
                    tool_call_id = %waiter.tool_call_id,
                    error = %e,
                    "failed to answer parked sub-chat tool call; dropping waiter"
                ),
            }
        }
    }

    /// Subchat ids with at least one waiter (observability/tests).
    pub async fn watched_ids(&self) -> Vec<i64> {
        self.waiters.read().await.keys().copied().collect()
    }

    /// Cheap pre-filter for the monitor hook: is this chat watched at all?
    async fn is_watched(&self, chat_id: i64) -> bool {
        self.waiters.read().await.contains_key(&chat_id)
    }
}

/// Build a [`ChatState`] from a `getChat` response, mirroring the monitor's
/// own refetch mapping (`chat_monitor.rs`).
pub(crate) fn chat_state_from_get(result: &GetChatResult) -> ChatState {
    ChatState {
        chat_id: result.chat.id,
        messages: result.messages.clone(),
        queued_messages_count: result.queued_messages_count,
        tags: result.chat.tags.clone(),
        version: result.chat.version,
    }
}

/// Install the single `ChatMonitor` hook fanning every watched chat's state
/// change into `watcher`. Call once at startup, after the monitor is
/// constructed and before the keep-alive loop.
pub async fn install(chat_monitor: &ChatMonitor, watcher: Arc<Watcher>) {
    chat_monitor
        .on_chat_state_change(move |chat_id, state| {
            let watcher = Arc::clone(&watcher);
            // NEVER await inline in the monitor's dispatch path — spawn (see
            // memory/development.md "Event Callbacks Must Not Block").
            tokio::spawn(async move {
                // Cheap read-lock filter first: virtually every chat in the
                // system is unwatched; the predicate only runs for ours. The
                // event-carried state is already version-consistent (the
                // monitor refetches on every change), so do NOT refetch here.
                if !watcher.is_watched(chat_id).await {
                    return;
                }
                if completion::is_completed(&state) {
                    watcher.drain_and_answer(chat_id, &state).await;
                }
            });
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhd_chat_api::{methods::GetChatStatus, Chat, Message};

    fn test_message(id: i64, role: &str, content: &str) -> Message {
        Message {
            id,
            chat_id: 42,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            created_at: "2026-09-23T00:00:00Z".parse().unwrap(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        }
    }

    /// Every ChatState field must be sourced from the right place in the
    /// `getChat` response (tags/version from `chat`, history + queue count
    /// from the top level).
    #[test]
    fn chat_state_from_get_maps_every_field() {
        let result = GetChatResult {
            chat: Chat {
                id: 42,
                title: "sub · x".to_string(),
                created_at: "2026-09-23T00:00:00Z".parse().unwrap(),
                updated_at: "2026-09-23T00:00:01Z".parse().unwrap(),
                tags: vec!["parent:7".to_string(), "root:7".to_string()],
                version: 9,
            },
            messages: vec![
                test_message(1, "user", "q"),
                test_message(2, "assistant", "a"),
            ],
            queued_messages_count: 3,
            status: Some(GetChatStatus::Actual),
        };

        let state = chat_state_from_get(&result);
        assert_eq!(state.chat_id, 42);
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[1].content, "a");
        assert_eq!(state.queued_messages_count, 3);
        assert_eq!(
            state.tags,
            vec!["parent:7".to_string(), "root:7".to_string()]
        );
        assert_eq!(state.version, 9);
    }

    /// The mapped state must feed the predicate consistently: a completed
    /// `getChat` response yields a completed ChatState.
    #[test]
    fn chat_state_from_get_feeds_completion_predicate() {
        let mut result = GetChatResult {
            chat: Chat {
                id: 42,
                title: "t".to_string(),
                created_at: "2026-09-23T00:00:00Z".parse().unwrap(),
                updated_at: "2026-09-23T00:00:00Z".parse().unwrap(),
                tags: vec![],
                version: 1,
            },
            messages: vec![test_message(1, "assistant", "done")],
            queued_messages_count: 0,
            status: None,
        };
        assert!(completion::is_completed(&chat_state_from_get(&result)));

        result.chat.tags.push("ai_completions:running".to_string());
        assert!(!completion::is_completed(&chat_state_from_get(&result)));
    }

    /// `Waiter` is a plain value: clone + Debug shapes the map storage.
    #[test]
    fn waiter_is_clonable_debuggable_value() {
        let waiter = Waiter {
            parent_chat_id: 7,
            tool_call_id: "call_x".to_string(),
        };
        let copy = waiter.clone();
        assert_eq!(copy.parent_chat_id, 7);
        assert_eq!(copy.tool_call_id, "call_x");
        assert!(format!("{waiter:?}").contains("call_x"));
    }
}
