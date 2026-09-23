//! Tool-call event handling: dispatch over the three sub-chat tool names and
//! the `rhd_sub_chat` spawn flow (validate → converge → answer / park).
//!
//! The dispatcher in `rhd_chat_client` already runs subscription callbacks in
//! spawned tasks, so awaiting client requests inline here is safe.

use std::sync::Arc;

use rhd_chat_api::{AssistantMessageWithToolCallsData, ToolCall};
use rhd_chat_client::ChatClient;

use crate::reply::AnswerGuards;
use crate::spawn::{SpawnError, SpawnPlan};
use crate::watcher::{Waiter, Watcher};
use crate::{reply, spawn};

/// Shared context for tool-call handlers.
#[derive(Clone)]
pub struct HandlerCtx {
    pub client: Arc<ChatClient>,
    pub watcher: Arc<Watcher>,
    pub guards: Arc<AnswerGuards>,
}

/// Handle one `rhd_sub_chat` call inside an assistant message.
///
/// Returns `Ok(())` — errors are answered (or logged), never propagated:
/// a failed answer is only logged, the tool loop simply stays parked.
pub async fn handle_spawn(ctx: HandlerCtx, caller_chat_id: i64, tool_call: ToolCall) {
    let tool_call_id = tool_call.id;

    // a. Duplicate-answer guard: an answer for this call already exists
    // (live event redelivery or an overlapping recovery pass) → skip.
    match reply::has_tool_result(&ctx.client, caller_chat_id, &tool_call_id).await {
        Ok(true) => {
            tracing::debug!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                "tool call already answered, skipping"
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            // Without a reliable guard, answering could duplicate; bail out
            // and let the next event / recovery pass retry.
            tracing::error!(
                chat_id = caller_chat_id,
                tool_call_id = %tool_call_id,
                error = %e,
                "failed to check for existing tool result, skipping spawn"
            );
            return;
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
            answer_error(&ctx, caller_chat_id, &tool_call_id, &message).await;
            return;
        }
    };

    // c. Build the replayable plan.
    let plan = SpawnPlan {
        caller_chat_id,
        tool_call_id: tool_call_id.clone(),
        request,
    };

    // d. Drive the two-phase activation to completion (idempotent).
    let sub_chat_id = match spawn::converge_spawn(&ctx.client, &plan).await {
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
            answer_error(&ctx, caller_chat_id, &tool_call_id, &message).await;
            return;
        }
    };

    // e. Answer per the Wire Contract. Every answer goes through the shared
    // AnswerGuards, so overlapping events and recovery passes can never
    // double-post for the same tool call.
    if plan.request.spawn_async {
        let notice = spawn::async_notice(sub_chat_id);
        match ctx
            .guards
            .answer_once(&ctx.client, caller_chat_id, &tool_call_id, notice)
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
    }
}

/// Entry point for the `on_tool_call` subscription (Phase 5 extends the
/// dispatch with the status/await arms).
pub async fn handle_tool_call_event(ctx: HandlerCtx, event: AssistantMessageWithToolCallsData) {
    for tool_call in &event.message.tool_calls {
        match tool_call.function.name.as_str() {
            "rhd_sub_chat" => handle_spawn(ctx.clone(), event.chat_id, tool_call.clone()).await,
            // `rhd_sub_chat_status` / `rhd_sub_chat_await` are already covered
            // by the subscription's name filter but handled only in Phase 5 —
            // until then their calls fall through unanswered (parked).
            _ => {}
        }
    }
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

/// Answer the tool call with a single-line `rhd_sub_chat error: ` message
/// (validation / mid-spawn failures) through the shared guards; a failed
/// answer is only logged.
async fn answer_error(ctx: &HandlerCtx, chat_id: i64, tool_call_id: &str, message: &str) {
    match ctx
        .guards
        .answer_once(
            ctx.client.as_ref(),
            chat_id,
            tool_call_id,
            format!("rhd_sub_chat error: {message}"),
        )
        .await
    {
        Ok(true) | Ok(false) => {}
        Err(e) => tracing::error!(
            chat_id,
            tool_call_id = %tool_call_id,
            error = %e,
            "failed to answer rhd_sub_chat with an error"
        ),
    }
}
