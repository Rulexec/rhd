//! Tool-call event handling: dispatch over the three sub-chat tool names —
//! the `rhd_sub_chat` spawn flow (validate → converge → answer / park) and the
//! `rhd_sub_chat_status` / `rhd_sub_chat_await` flows (shared direct-child
//! lookup → fresh predicate evaluation / watcher registration).
//!
//! Layout (Phase 6): this module holds the **entry points** — each one takes
//! the outermost processing claim on its `tool_call_id` and delegates to the
//! unclaimed processing code in [`flows`]. The per-tool status/await
//! functions are plain (no `ToolCall` coupling): Phase 6 recovery calls these
//! exact signatures — this is the frozen seam — and, already holding its own
//! claim, calls the same `flows::*` functions directly, never re-claiming the
//! id (plan note 1: claim outermost, exactly once).
//!
//! The dispatcher in `rhd_chat_client` already runs subscription callbacks in
//! spawned tasks, so awaiting client requests inline here is safe.

use std::sync::Arc;

use rhd_chat_api::{AssistantMessageWithToolCallsData, ToolCall};
use rhd_chat_client::ChatClient;

use crate::reply::AnswerGuards;
use crate::watcher::Watcher;

pub(crate) mod flows;

/// Shared context for tool-call handlers.
#[derive(Clone)]
pub struct HandlerCtx {
    pub client: Arc<ChatClient>,
    pub watcher: Arc<Watcher>,
    pub guards: Arc<AnswerGuards>,
}

/// Claim `tool_call_id` for whole-call processing, or skip silently: another
/// live handler or the recovery scan owns it and will answer / register. The
/// RAII guard released on every exit path of the flow.
macro_rules! claim_or_skip {
    ($ctx:expr, $chat_id:expr, $tool_call_id:expr) => {
        match $ctx.guards.claim_guard($tool_call_id) {
            Some(claim) => claim,
            None => {
                tracing::debug!(
                    chat_id = $chat_id,
                    tool_call_id = $tool_call_id,
                    "sub-chat call already being processed elsewhere, skipping"
                );
                return;
            }
        }
    };
}

/// Handle one `rhd_sub_chat` call inside an assistant message.
///
/// Returns `Ok(())` — errors are answered (or logged), never propagated:
/// a failed answer is only logged, the tool loop simply stays parked.
pub async fn handle_spawn(ctx: HandlerCtx, caller_chat_id: i64, tool_call: ToolCall) {
    let _claim = claim_or_skip!(&ctx, caller_chat_id, &tool_call.id);
    flows::spawn_flow(&ctx, caller_chat_id, &tool_call).await;
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
    let _claim = claim_or_skip!(ctx, caller_chat_id, tool_call_id);
    flows::status_flow(ctx, tool, caller_chat_id, tool_call_id, target_chat_id).await;
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
    let _claim = claim_or_skip!(ctx, caller_chat_id, tool_call_id);
    flows::await_flow(ctx, tool, caller_chat_id, tool_call_id, target_chat_id).await;
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
                        flows::answer_error(
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

#[cfg(test)]
mod tests;
