//! The unclaimed processing behind every entry point in
//! [`super`](crate::handler): live handlers reach these through the
//! thin claiming wrappers, and Phase 6 recovery calls them **directly** after
//! taking the outermost claim itself — one claim per logical flow, never
//! re-claimed for the same `tool_call_id` (plan note 1). Sharing the exact
//! same code is what makes recovery's answers byte-identical to the live
//! path's by construction.
//!
//! The flows return small outcome enums purely so the recovery report can
//! classify what happened; the live entries ignore them.

use rhd_chat_api::{GetChatParams, ToolCall};
use rhd_chat_client::{ChatClient, ChatState, ClientError};

use crate::completion;
use crate::spawn::{self, SpawnError, SpawnPlan};
use crate::watcher::{self, Waiter};
use crate::{reply, tags};

use super::HandlerCtx;

/// What the shared `rhd_sub_chat` processing ended up doing with one call.
/// The live path ignores it; recovery maps it onto the `RecoveryReport`
/// counters (combined with the link-tag index snapshot).
pub(crate) enum SpawnFlow {
    /// A persisted answer already existed when processing began — nothing
    /// done (counts as `skipped_answered`).
    AlreadyAnswered,
    /// The persisted-guard read failed, so touching the call would risk a
    /// duplicate answer — nothing done, retriable later; logged inside.
    GuardUnavailable,
    /// Bad arguments → validation error answered (AD-13, no chat created).
    Rejected,
    /// Converge failed → error answered (B stays `paused` when it exists).
    Failed,
    /// Converged; the async background notice was answered (this is also the
    /// late-notice case after a crash before the notice — self-healing).
    Async { sub_chat_id: i64 },
    /// Converged; a sync waiter was registered (or answered immediately by
    /// `register_or_complete`'s own evaluation if the subchat is already done).
    Sync { sub_chat_id: i64 },
}

/// Drive one unresolved `rhd_sub_chat` call: duplicate guard → parse →
/// converge → answer / park. Assumes the caller holds the processing claim
/// for `tool_call.id` (entry point or recovery scan).
pub(crate) async fn spawn_flow(
    ctx: &HandlerCtx,
    caller_chat_id: i64,
    tool_call: &ToolCall,
) -> SpawnFlow {
    let tool_call_id = &tool_call.id;

    // a. Duplicate-answer guard: an answer for this call already exists
    // (live event redelivery or an overlapping recovery pass) → skip. The
    // claim above already excludes concurrent processing; this is the
    // persisted second line of defense.
    match reply::has_tool_result(ctx.client.as_ref(), caller_chat_id, tool_call_id).await {
        Ok(true) => {
            tracing::debug!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                "tool call already answered, skipping"
            );
            return SpawnFlow::AlreadyAnswered;
        }
        Ok(false) => {}
        Err(e) => {
            // Without a reliable guard, answering could duplicate; bail out
            // and let the next event / recovery pass retry the call.
            tracing::error!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                error = %e,
                "failed to check for existing tool result, skipping spawn"
            );
            return SpawnFlow::GuardUnavailable;
        }
    }

    // b. Parse arguments. They arrive as a raw provider string, so any parse
    // failure is the model's problem → a Validation error answer (AD-13).
    let request = match spawn::parse_spawn_request(&tool_call.function.arguments) {
        Ok(request) => request,
        Err(e) => {
            let message = match e {
                SpawnError::Validation(message) => message,
                unexpected => {
                    tracing::error!(
                        chat_id = caller_chat_id,
                        tool_call_id = %tool_call_id,
                        error = %unexpected,
                        "unexpected error parsing spawn arguments"
                    );
                    format!("unexpected error: {unexpected}")
                }
            };
            answer_error(ctx, "rhd_sub_chat", caller_chat_id, tool_call_id, &message).await;
            return SpawnFlow::Rejected;
        }
    };

    // c. Build the replayable plan.
    let plan = SpawnPlan {
        caller_chat_id,
        tool_call_id: tool_call_id.clone(),
        request,
    };

    // d. Drive the two-phase activation to completion (idempotent).
    let sub_chat_id = match spawn::converge_spawn(ctx.client.as_ref(), &plan).await {
        Ok(sub_chat_id) => sub_chat_id,
        Err(e) => {
            tracing::warn!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                error = %e,
                "subchat spawn failed"
            );
            let message = match &e {
                // converge never validates (arguments were checked in b),
                // but keep the mapping total.
                SpawnError::Validation(message) => message.clone(),
                SpawnError::Unreconcilable { sub_chat_id } => prepare_failure_answer(
                    Some(*sub_chat_id),
                    "queued starting messages no longer match the tool call arguments",
                ),
                SpawnError::Server {
                    sub_chat_id,
                    detail,
                } => prepare_failure_answer(*sub_chat_id, detail),
            };
            answer_error(ctx, "rhd_sub_chat", caller_chat_id, tool_call_id, &message).await;
            return SpawnFlow::Failed;
        }
    };

    // e. Answer per the Wire Contract. Every answer goes through the shared
    // AnswerGuards, so overlapping events and recovery passes can never
    // double-post for the same tool call.
    if plan.request.spawn_async {
        let notice = spawn::async_notice(sub_chat_id);
        match ctx
            .guards
            .answer_once(ctx.client.as_ref(), caller_chat_id, tool_call_id, notice)
            .await
        {
            Ok(true) => {}
            Ok(false) => tracing::debug!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                "async answer skipped as duplicate"
            ),
            Err(e) => tracing::error!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                error = %e,
                "failed to answer async rhd_sub_chat"
            ),
        }
        SpawnFlow::Async { sub_chat_id }
    } else {
        // Sync mode (AD-1): park the parent's loop by registering a waiter on
        // the subchat. The completion watcher answers it — either from the
        // monitor state-change hook or from the immediate evaluate inside
        // register_or_complete, which closes the race where the subchat
        // finished while this handler was still converging.
        ctx.watcher
            .register_or_complete(
                sub_chat_id,
                Waiter {
                    parent_chat_id: caller_chat_id,
                    tool_call_id: tool_call_id.clone(),
                },
            )
            .await;
        SpawnFlow::Sync { sub_chat_id }
    }
}

/// What a claimed `rhd_sub_chat_await` call ended up as (`status` always
/// answers, so its flow needs no outcome).
pub(crate) enum AwaitFlow {
    /// The target was completed (or the call was rejected with an error
    /// answer) — the call is resolved.
    Answered,
    /// The target is still running: a watcher waiter was registered and the
    /// call stays parked (counts as `rewatched` in recovery).
    Parked,
}

/// Handle a claimed `rhd_sub_chat_status` call: answer a **fresh** evaluation
/// of the completion predicate (see [`super::handle_status`] for the
/// contract). The call always ends answered — pending, completed, or error.
pub(crate) async fn status_flow(
    ctx: &HandlerCtx,
    tool: &str,
    caller_chat_id: i64,
    tool_call_id: &str,
    target_chat_id: i64,
) {
    let state = match ensure_direct_child(ctx.client.as_ref(), caller_chat_id, target_chat_id).await
    {
        Ok(state) => state,
        Err(message) => {
            answer_error(ctx, tool, caller_chat_id, tool_call_id, &message).await;
            return;
        }
    };
    let text = if completion::is_completed(&state) {
        completion::COMPLETED
    } else {
        completion::PENDING
    };
    answer_guarded(ctx, caller_chat_id, tool_call_id, text.to_string()).await;
}

/// Handle a claimed `rhd_sub_chat_await` call (see [`super::handle_await`]
/// for the contract).
pub(crate) async fn await_flow(
    ctx: &HandlerCtx,
    tool: &str,
    caller_chat_id: i64,
    tool_call_id: &str,
    target_chat_id: i64,
) -> AwaitFlow {
    let state = match ensure_direct_child(ctx.client.as_ref(), caller_chat_id, target_chat_id).await
    {
        Ok(state) => state,
        Err(message) => {
            answer_error(ctx, tool, caller_chat_id, tool_call_id, &message).await;
            return AwaitFlow::Answered;
        }
    };
    if completion::is_completed(&state) {
        let content = completion::final_answer(&state)
            .unwrap_or_default()
            .to_string();
        answer_guarded(ctx, caller_chat_id, tool_call_id, content).await;
        return AwaitFlow::Answered;
    }
    ctx.watcher
        .register_or_complete(
            target_chat_id,
            Waiter {
                parent_chat_id: caller_chat_id,
                tool_call_id: tool_call_id.to_string(),
            },
        )
        .await;
    AwaitFlow::Parked
}

/// Fetch a chat and enforce the direct-child rule (AD-5): the target must
/// carry the exact `parent:<caller>` tag; siblings, grandchildren, unrelated
/// and unknown chats are rejected. The `Err` strings are ready-to-send error
/// answers — callers only add the `{tool} error: ` prefix.
async fn ensure_direct_child(
    client: &ChatClient,
    caller_chat_id: i64,
    target_chat_id: i64,
) -> Result<ChatState, String> {
    match client
        .get_chat(GetChatParams {
            chat_id: target_chat_id,
            if_version_higher_than: None,
        })
        .await
    {
        Err(e) if is_not_found(&e) => Err(format!("chat {target_chat_id} not found")),
        Err(e) => Err(format!("chat {target_chat_id} lookup failed: {e}")),
        Ok(result) if !tags::is_direct_child(&result.chat.tags, caller_chat_id) => Err(format!(
            "chat {target_chat_id} is not a direct subchat of this chat"
        )),
        Ok(result) => Ok(watcher::chat_state_from_get(&result)),
    }
}

/// Only the server's structured `CHAT_NOT_FOUND` code (see
/// `rhd_chat_api::ErrorCode`, surfaced by the client as `ClientError::Server`)
/// means "this chat does not exist". Everything else — timeouts, connection
/// drops, other codes — is a lookup failure, deliberately kept distinct from
/// "not found". The whole mapping lives here, in one place.
fn is_not_found(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::Server { code, .. }
            if *code == rhd_chat_api::ErrorCode::ChatNotFound.to_string()
    )
}

/// Mid-spawn failure answer (Wire Contract). The "(subchat left paused for
/// recovery)" suffix applies only when a subchat id exists — a failed create
/// leaves nothing behind and uses the `new` fallback without the suffix
/// (implementation note 5).
fn prepare_failure_answer(sub_chat_id: Option<i64>, detail: &str) -> String {
    match sub_chat_id {
        Some(id) => format!(
            "rhd_sub_chat error: failed to prepare subchat {id}: {detail} (subchat left paused for recovery)"
        ),
        None => format!("rhd_sub_chat error: failed to prepare subchat new: {detail}"),
    }
}

/// Answer the tool call with a single-line `{tool} error: ` message (spawn
/// validation / mid-spawn failures, status/await rejections, invalid
/// arguments) through the shared guards. Also used by the recovery scan for
/// unparseable status/await arguments, so the error wording lives in exactly
/// one place.
pub(crate) async fn answer_error(
    ctx: &HandlerCtx,
    tool: &str,
    chat_id: i64,
    tool_call_id: &str,
    message: &str,
) {
    answer_guarded(ctx, chat_id, tool_call_id, format!("{tool} error: {message}")).await;
}

/// Post `content` as the answer through the shared guards (which skip
/// in-flight and already-persisted duplicates). A failed answer is only
/// logged: the caller's loop stays parked and a later event or the Phase 6
/// recovery pass retries the call.
async fn answer_guarded(ctx: &HandlerCtx, chat_id: i64, tool_call_id: &str, content: String) {
    match ctx
        .guards
        .answer_once(ctx.client.as_ref(), chat_id, tool_call_id, content)
        .await
    {
        Ok(true) | Ok(false) => {}
        Err(e) => tracing::error!(
            chat_id,
            tool_call_id = %tool_call_id,
            error = %e,
            "failed to answer sub-chat tool call"
        ),
    }
}
