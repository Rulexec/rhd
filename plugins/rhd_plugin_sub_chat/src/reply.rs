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
///
/// Beyond answer dedup, the `processing` claim set serializes **whole call
/// handling** (Phase 6): answer dedup alone cannot stop two overlapping
/// **spawns** (create/queue side effects), so every live entry point and the
/// recovery scan claims the `tool_call_id` for the full duration of its
/// processing — the loser of the claim returns silently, the owner answers
/// or registers.
#[derive(Default)]
pub struct AnswerGuards {
    answered: tokio::sync::Mutex<HashSet<String>>,
    /// tool_call_ids currently being processed by a live handler or the scan.
    ///
    /// A **std** mutex rather than `tokio::sync` (as the phase plan sketched):
    /// the set is touched only by synchronous insert/remove — never across an
    /// `await` — and the RAII [`ProcessingClaim`] must release in `Drop`,
    /// which runs on a runtime worker thread where `tokio::sync::Mutex`'s
    /// blocking lock panics. Poisoning cannot arise in practice (the tiny
    /// critical sections never panic), but `into_inner` keeps a foreign panic
    /// from bricking every later claim.
    processing: std::sync::Mutex<HashSet<String>>,
}

impl AnswerGuards {
    /// Try to own the processing of `tool_call_id`. `false` → someone else is
    /// on it; skip the whole call (the owner will answer or register).
    pub fn claim(&self, tool_call_id: &str) -> bool {
        self.processing().insert(tool_call_id.to_string())
    }

    /// Give up processing ownership. Until this runs (or the process dies),
    /// no other path — live event or recovery scan — touches the id.
    pub fn release(&self, tool_call_id: &str) {
        self.processing().remove(tool_call_id);
    }

    /// RAII [`claim`](Self::claim): releases on drop, so every early return
    /// (and panic-unwind) path through a handler gives the id back. Entry
    /// points take the claim **exactly once, outermost** — nested processing
    /// (converge, flows called from the recovery scan) receives the claim
    /// holder's context, never re-claims its own id (plan note 1).
    /// `None` when the id is already owned elsewhere.
    pub fn claim_guard(&self, tool_call_id: &str) -> Option<ProcessingClaim<'_>> {
        self.claim(tool_call_id).then(|| ProcessingClaim {
            guards: self,
            tool_call_id: tool_call_id.to_string(),
        })
    }

    fn processing(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.processing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

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

/// Borrowed claim on one `tool_call_id`; dropping releases it. See
/// [`AnswerGuards::claim_guard`].
#[must_use = "a claim guard must be held for the whole processing, not dropped immediately"]
pub struct ProcessingClaim<'a> {
    guards: &'a AnswerGuards,
    tool_call_id: String,
}

impl Drop for ProcessingClaim<'_> {
    fn drop(&mut self) {
        self.guards.release(&self.tool_call_id);
    }
}

#[cfg(test)]
mod tests {
    use super::AnswerGuards;

    /// The claim set serializes processing: first claim wins, the second
    /// claimant is refused, and `release` opens the id up again.
    #[test]
    fn claim_is_exclusive_until_released() {
        let guards = AnswerGuards::default();
        assert!(guards.claim("tc1"));
        assert!(!guards.claim("tc1"), "double claim of the same id refused");
        assert!(guards.claim("tc2"), "other ids are unaffected");
        guards.release("tc1");
        assert!(guards.claim("tc1"), "released id is claimable again");
    }

    /// The RAII guard releases on scope exit — including the early returns
    /// every handler is full of — and only touches its own id.
    #[test]
    fn claim_guard_releases_on_drop() {
        let guards = AnswerGuards::default();
        let claim = guards
            .claim_guard("tcX")
            .expect("fresh id claims");
        assert!(!guards.claim("tcX"));
        guards.release("tcY"); // releasing an unheld id is a no-op, not a panic
        {
            let nested = guards.claim_guard("tcZ");
            assert!(nested.is_some(), "concurrent ids claim independently");
        }
        assert!(guards.claim("tcZ"), "nested guard released on drop");
        guards.release("tcZ");
        drop(claim);
        assert!(guards.claim_guard("tcX").is_some(), "outer guard released");
    }

    /// A refused claim yields no guard, hence no phantom release of the
    /// holder's claim.
    #[test]
    fn refused_claim_leaves_the_owner_untouched() {
        let guards = AnswerGuards::default();
        let owner = guards.claim_guard("tc1").expect("first claim wins");
        assert!(guards.claim_guard("tc1").is_none());
        drop(owner);
        assert!(guards.claim("tc1"), "only the real release freed the id");
    }
}
