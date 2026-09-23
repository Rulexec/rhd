//! Tool-call event handling: dispatch over the three sub-chat tool names —
//! the `rhd_sub_chat` spawn flow (validate → converge → answer / park) and the
//! `rhd_sub_chat_status` / `rhd_sub_chat_await` flows (shared direct-child
//! lookup → fresh predicate evaluation / watcher registration).
//!
//! The per-tool status/await functions are plain (no `ToolCall` coupling):
//! Phase 6 recovery calls these exact signatures with values parsed from
//! stored arguments — this is the frozen seam.
//!
//! The dispatcher in `rhd_chat_client` already runs subscription callbacks in
//! spawned tasks, so awaiting client requests inline here is safe.

use std::sync::Arc;

use rhd_chat_api::{AssistantMessageWithToolCallsData, GetChatParams, ToolCall};
use rhd_chat_client::{ChatClient, ChatState, ClientError};

use crate::completion::{self, COMPLETED, PENDING};
use crate::reply::AnswerGuards;
use crate::spawn::{SpawnError, SpawnPlan};
use crate::watcher::{self, Waiter, Watcher};
use crate::{reply, spawn, tags};

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
            answer_error(&ctx, "rhd_sub_chat", caller_chat_id, &tool_call_id, &message).await;
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
            answer_error(&ctx, "rhd_sub_chat", caller_chat_id, &tool_call_id, &message).await;
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

/// Parse the `{"chatId": int}` arguments of the status/await tools. The error
/// string is a single-line, model-facing fragment; the dispatcher prefixes it
/// with `{name} error: invalid arguments: ` (Wire Contract). Strict on
/// purpose: unparseable JSON, non-object documents, a missing key, and
/// non-integer values (strings, floats, nulls, bools) all fail. Unknown extra
/// fields are tolerated like in `spawn::parse_spawn_request`. (An explicit
/// `Value` walk rather than a derived struct: serde derives also accept a
/// JSON **array** as a struct in sequence form, which the contract forbids.)
pub fn parse_target_chat_id(arguments: &str) -> Result<i64, String> {
    let value: serde_json::Value =
        serde_json::from_str(arguments).map_err(|e| e.to_string())?;
    value.as_object().ok_or_else(|| {
        r#"expected a JSON object like {"chatId": <id>}"#.to_string()
    })?;
    let chat_id = value
        .get("chatId")
        .ok_or_else(|| "missing field `chatId`".to_string())?;
    chat_id
        .as_i64()
        .ok_or_else(|| "`chatId` must be an integer".to_string())
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

/// Handle a `rhd_sub_chat_status` call: answer a **fresh** evaluation of the
/// completion predicate with the literal `pending` / `completed` (AD-6: no
/// reason suffix; note 1: the watcher cache is never consulted — "status"
/// means right now, so a re-activated subchat reads `pending` again).
pub async fn handle_status(
    ctx: &HandlerCtx,
    tool: &str,
    caller_chat_id: i64,
    tool_call_id: &str,
    target_chat_id: i64,
) {
    let state = match ensure_direct_child(&ctx.client, caller_chat_id, target_chat_id).await {
        Ok(state) => state,
        Err(message) => {
            answer_error(ctx, tool, caller_chat_id, tool_call_id, &message).await;
            return;
        }
    };
    let text = if completion::is_completed(&state) {
        COMPLETED
    } else {
        PENDING
    };
    answer_guarded(ctx, caller_chat_id, tool_call_id, text.to_string()).await;
}

/// Handle a `rhd_sub_chat_await` call: completed target → final assistant
/// content verbatim, answered immediately; otherwise register a watcher
/// waiter — an `await` waiter is indistinguishable from a sync-spawn waiter
/// inside `Watcher` (that reuse is the point). Not answering *is* the parking
/// mechanism (AD-1). Note 2: awaiting a completed-then-reactivated subchat
/// registers and waits for the **next** idle transition, no special casing.
pub async fn handle_await(
    ctx: &HandlerCtx,
    tool: &str,
    caller_chat_id: i64,
    tool_call_id: &str,
    target_chat_id: i64,
) {
    let state = match ensure_direct_child(&ctx.client, caller_chat_id, target_chat_id).await {
        Ok(state) => state,
        Err(message) => {
            answer_error(ctx, tool, caller_chat_id, tool_call_id, &message).await;
            return;
        }
    };
    if completion::is_completed(&state) {
        let content = completion::final_answer(&state).unwrap_or_default().to_string();
        answer_guarded(ctx, caller_chat_id, tool_call_id, content).await;
        return;
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
}

/// Entry point for the `on_tool_call` subscription: dispatch over the three
/// sub-chat tool names.
pub async fn handle_tool_call_event(ctx: HandlerCtx, event: AssistantMessageWithToolCallsData) {
    for tool_call in &event.message.tool_calls {
        let name = tool_call.function.name.as_str();
        match name {
            "rhd_sub_chat" => handle_spawn(ctx.clone(), event.chat_id, tool_call.clone()).await,
            "rhd_sub_chat_status" | "rhd_sub_chat_await" => {
                match parse_target_chat_id(&tool_call.function.arguments) {
                    Ok(target_chat_id) if name == "rhd_sub_chat_status" => {
                        handle_status(&ctx, name, event.chat_id, &tool_call.id, target_chat_id)
                            .await;
                    }
                    Ok(target_chat_id) => {
                        handle_await(&ctx, name, event.chat_id, &tool_call.id, target_chat_id)
                            .await;
                    }
                    Err(message) => {
                        answer_error(
                            &ctx,
                            name,
                            event.chat_id,
                            &tool_call.id,
                            &format!("invalid arguments: {message}"),
                        )
                        .await;
                    }
                }
            }
            // Foreign names never reach us in practice — the subscription
            // filters on the three tool names (Phase 3) — but stay parked.
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

/// Answer the tool call with a single-line `{tool} error: ` message (spawn
/// validation / mid-spawn failures, status/await rejections, invalid
/// arguments) through the shared guards.
async fn answer_error(ctx: &HandlerCtx, tool: &str, chat_id: i64, tool_call_id: &str, message: &str) {
    answer_guarded(
        ctx,
        chat_id,
        tool_call_id,
        format!("{tool} error: {message}"),
    )
    .await;
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

#[cfg(test)]
mod tests {
    use super::parse_target_chat_id;

    #[test]
    fn parses_the_contract_shape() {
        assert_eq!(parse_target_chat_id(r#"{"chatId":5}"#), Ok(5));
    }

    #[test]
    fn rejects_missing_key() {
        assert!(parse_target_chat_id("{}").is_err());
        // The error fragment names the missing field for the model.
        let err = parse_target_chat_id("{}").unwrap_err();
        assert!(err.contains("chatId"), "error mentions the key: {err}");
    }

    #[test]
    fn rejects_non_integer_values() {
        for arguments in [
            r#"{"chatId":"5"}"#,
            r#"{"chatId":5.5}"#,
            r#"{"chatId":null}"#,
            r#"{"chatId":true}"#,
        ] {
            assert!(
                parse_target_chat_id(arguments).is_err(),
                "expected rejection of {arguments}"
            );
        }
    }

    #[test]
    fn rejects_non_object_and_unparseable_json() {
        for arguments in [
            "",
            "not json at all",
            "5",
            "\"chat\"",
            "[5]",
            "[{\"chatId\":5}]",
        ] {
            assert!(
                parse_target_chat_id(arguments).is_err(),
                "expected rejection of {arguments:?}"
            );
        }
    }

    #[test]
    fn tolerates_extra_fields_like_spawn_parsing() {
        assert_eq!(parse_target_chat_id(r#"{"chatId":42,"junk":true}"#), Ok(42));
    }
}
