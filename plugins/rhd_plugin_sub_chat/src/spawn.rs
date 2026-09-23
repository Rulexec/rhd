//! Spawn engine: request types, pure validation helpers, and the idempotent
//! converge primitive implementing the two-phase activation protocol
//! (AD-3/AD-4/AD-13): create paused with the full atomic tag set → reconcile +
//! enqueue starting messages → remove `paused` last.
//!
//! `converge_spawn` is the recovery kernel — it only manipulates chats and
//! never answers tool calls; Phase 6 recovery calls this very function.

use rhd_chat_api::{
    AddQueueMessageParams, CreateChatParams, GetChatParams, GetQueueMessagesParams,
    ListChatsParams, Message, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, ClientError};

use crate::tags;

/// Max characters of the first message taken into a generated title (AD-10).
const MAX_TITLE_CHARS: usize = 40;
/// Prefix of every generated subchat title.
const TITLE_PREFIX: &str = "sub · ";
/// Title body when there is no usable message content at all.
const TITLE_FALLBACK: &str = "subchat";

/// One starting (seed) message for a subchat; roles restricted to system/user.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StartingMessage {
    pub role: String,
    pub content: String,
}

/// Validated `rhd_sub_chat` tool-call arguments.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SpawnRequest {
    pub messages: Vec<StartingMessage>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "async")]
    pub spawn_async: bool,
}

/// Immutable description of one rhd_sub_chat call, replayable at any time
/// (live handler or Phase 6 recovery — the tool-call arguments stored in the
/// parent chat are the only source of truth).
#[derive(Debug, Clone)]
pub struct SpawnPlan {
    pub caller_chat_id: i64,
    pub tool_call_id: String,
    pub request: SpawnRequest,
}

/// Errors produced while spawning. The variant decides the answer contract
/// (AD-13 / Wire Contract): `Validation` never creates a chat, while
/// `Unreconcilable` and `Server` may reference an existing (still-paused)
/// subchat id so the caller can point recovery at it.
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    /// Bad arguments → error-answer, no chat created.
    #[error("validation: {0}")]
    Validation(String),
    /// Queue no longer matches the plan prefix while `paused` (human
    /// tampering) → refuse to guess, leave the subchat paused (AD-13).
    #[error("cannot reconcile paused subchat {sub_chat_id}")]
    Unreconcilable { sub_chat_id: i64 },
    /// Any client-call failure, with context. `sub_chat_id` is `Some` once the
    /// subchat exists (mid-spawn failure, left paused for recovery) and `None`
    /// when the create itself failed (nothing was left behind).
    #[error("server: {detail}")]
    Server {
        sub_chat_id: Option<i64>,
        detail: String,
    },
}

/// Parse and validate the JSON arguments string (roles ∈ {system, user},
/// non-empty messages, string contents, tags via `tags::validate_user_tags`).
/// Any failure — including unparseable JSON, since arguments arrive as a raw
/// provider string — is a `Validation` error.
pub fn parse_spawn_request(arguments: &str) -> Result<SpawnRequest, SpawnError> {
    let request: SpawnRequest = serde_json::from_str(arguments)
        .map_err(|e| SpawnError::Validation(format!("invalid arguments: {e}")))?;

    if request.messages.is_empty() {
        return Err(SpawnError::Validation(
            "messages must not be empty".to_string(),
        ));
    }
    for (i, message) in request.messages.iter().enumerate() {
        if message.role != "system" && message.role != "user" {
            return Err(SpawnError::Validation(format!(
                "messages[{i}]: role '{}' is not allowed (only 'system' and 'user')",
                message.role
            )));
        }
    }
    tags::validate_user_tags(&request.tags).map_err(SpawnError::Validation)?;

    Ok(request)
}

/// `sub · ` + first 40 chars of the first user message (fallback: first
/// message; fallback: "subchat"), `…` appended when truncated.
pub fn generated_title(messages: &[StartingMessage]) -> String {
    let source = messages
        .iter()
        .find(|m| m.role == "user")
        .or_else(|| messages.first())
        .map(|m| m.content.as_str())
        .filter(|content| !content.is_empty())
        .unwrap_or(TITLE_FALLBACK);

    // Take one char beyond the budget to detect truncation without a second pass.
    let mut probe: Vec<char> = source.chars().take(MAX_TITLE_CHARS + 1).collect();
    let truncated = probe.len() > MAX_TITLE_CHARS;
    probe.truncate(MAX_TITLE_CHARS);
    let body: String = probe.into_iter().collect();

    if truncated {
        format!("{TITLE_PREFIX}{body}…")
    } else {
        format!("{TITLE_PREFIX}{body}")
    }
}

/// The exact async notice (Wire Contract / AD-6) — golden-locked by tests.
pub fn async_notice(chat_id: i64) -> String {
    format!(
        "Chat started in background with id {chat_id}, use tools rhd_sub_chat_status or rhd_sub_chat_await on it"
    )
}

/// Indices of `expected` messages missing from the observed queue.
/// Queue items must equal `expected[i]` (role + content) in order; any queue
/// item beyond the expected length or any content mismatch → `Err(())`
/// (human tampering; refuse to guess).
pub fn plan_missing_suffix(
    queue: &[Message],
    expected: &[StartingMessage],
) -> Result<Vec<usize>, ()> {
    if queue.len() > expected.len() {
        return Err(());
    }
    for (i, queued) in queue.iter().enumerate() {
        let want = &expected[i];
        if queued.role != want.role || queued.content != want.content {
            return Err(());
        }
    }
    Ok((queue.len()..expected.len()).collect())
}

/// Converge the world to "subchat for `plan` exists, has all starting
/// messages queued, and is activated (not paused)". Returns the subchat id.
/// Safe to call any number of times from any crash point; NEVER unpauses
/// before the queue is complete.
pub async fn converge_spawn(client: &ChatClient, plan: &SpawnPlan) -> Result<i64, SpawnError> {
    // 1. Ensure caller lineage: the root is the caller's declared `root:<R>`,
    //    else the caller itself (which then gets `root:<A>` added).
    let caller = client
        .get_chat(GetChatParams {
            chat_id: plan.caller_chat_id,
            if_version_higher_than: None,
        })
        .await
        .map_err(|e| server_err("step 1 (read caller chat)", None, &e))?;
    let declared_root = tags::root_of(&caller.chat.tags);
    let root = declared_root.unwrap_or(plan.caller_chat_id);
    if declared_root.is_none() {
        client
            .update_chat(UpdateChatParams {
                chat_id: plan.caller_chat_id,
                title: None,
                add_tags: vec![tags::root_tag(plan.caller_chat_id)],
                remove_tags: vec![],
            })
            .await
            .map_err(|e| server_err("step 1 (tag caller root)", None, &e))?;
    }

    // 2. Find the existing subchat by its link tag, or create one with the
    //    FULL Wire-Contract tag set in the single `createChat` call — tags
    //    land before any queue item, structurally (AD-4).
    let link = tags::call_tag(&plan.tool_call_id);
    let existing = client
        .list_chats(ListChatsParams {
            tags: vec![link.clone()],
        })
        .await
        .map_err(|e| server_err("step 2 (list chats)", None, &e))?;
    let matches: Vec<_> = existing
        .chats
        .into_iter()
        .filter(|c| c.tags.iter().any(|t| t == &link))
        .collect();

    let (sub_chat_id, sub_chat_tags) = if matches.len() == 1 {
        let summary = &matches[0];
        tracing::info!(
            sub_chat_id = summary.id,
            tool_call_id = %plan.tool_call_id,
            "reusing existing subchat for tool call"
        );
        (summary.id, summary.tags.clone())
    } else {
        if matches.len() > 1 {
            tracing::warn!(
                tool_call_id = %plan.tool_call_id,
                matches = matches.len(),
                "multiple chats carry the link tag; creating a new subchat"
            );
        }
        // NOTE(Phase 6): create races — a live converge and the startup
        // recovery pass could both observe zero matches and both create.
        // Tolerable in Phase 3 (recovery does not exist yet); Phase 6
        // serializes via its single startup pass + answer guards.
        let mut new_tags = vec![
            tags::parent_tag(plan.caller_chat_id),
            tags::root_tag(root),
            link.clone(),
        ];
        new_tags.extend(plan.request.tags.iter().cloned());
        new_tags.push(tags::PAUSED_TAG.to_string());
        let created = client
            .create_chat(CreateChatParams {
                title: generated_title(&plan.request.messages),
                tags: new_tags.clone(),
            })
            .await
            .map_err(|e| server_err("step 2 (create subchat)", None, &e))?;
        tracing::info!(
            sub_chat_id = created.chat_id,
            tool_call_id = %plan.tool_call_id,
            "created paused subchat"
        );
        (created.chat_id, new_tags)
    };

    // 3–4. While the subchat still carries `paused` (fresh state from the step-2
    // fetch, not a stale cache): reconcile the queue prefix, enqueue the
    // missing suffix sequentially in order, then remove `paused` — the
    // activation point, strictly last.
    if tags::has_paused_tag(&sub_chat_tags) {
        let queue = client
            .get_queue_messages(GetQueueMessagesParams {
                chat_id: sub_chat_id,
            })
            .await
            .map_err(|e| server_err("step 3 (read queue)", Some(sub_chat_id), &e))?;
        let missing = plan_missing_suffix(&queue.messages, &plan.request.messages)
            .map_err(|()| SpawnError::Unreconcilable { sub_chat_id })?;
        for index in missing {
            let message = &plan.request.messages[index];
            // Sequential, in `messages` order — addQueueMessage appends by
            // position, so the provider history reads system→user in seed order.
            client
                .add_queue_message(AddQueueMessageParams {
                    chat_id: sub_chat_id,
                    role: message.role.clone(),
                    content: message.content.clone(),
                    tool_call_id: None,
                    reasoning_content: None,
                    tags: vec![],
                    before_message_id: None,
                })
                .await
                .map_err(|e| {
                    server_err(
                        &format!("step 3 (enqueue message {index})"),
                        Some(sub_chat_id),
                        &e,
                    )
                })?;
        }

        client
            .update_chat(UpdateChatParams {
                chat_id: sub_chat_id,
                title: None,
                add_tags: vec![],
                remove_tags: vec![tags::PAUSED_TAG.to_string()],
            })
            .await
            .map_err(|e| server_err("step 4 (activate subchat)", Some(sub_chat_id), &e))?;
        tracing::info!(sub_chat_id, "subchat activated");
    }

    Ok(sub_chat_id)
}

/// Wrap a client failure into `SpawnError::Server` with step context and the
/// subchat id when the chat already exists.
fn server_err(step: &str, sub_chat_id: Option<i64>, error: &ClientError) -> SpawnError {
    SpawnError::Server {
        sub_chat_id,
        detail: format!("{step}: {error}"),
    }
}

#[cfg(test)]
mod tests;
