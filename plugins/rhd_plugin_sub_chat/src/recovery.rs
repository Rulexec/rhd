//! Startup recovery (AD-7): re-drive every unfinished sub-chat tool call
//! from chat history alone — the zero-private-persistence promise.
//!
//! Runs once after the monitor subscription and the watcher hook are
//! installed. The scan asks each monitored chat for its messages with
//! unresolved tool calls (`getMessages` + `withUnresolvedToolCalls`, computed
//! server-side), keeps the three tool names we own, and re-drives each call
//! through **exactly the live-path code** — [`crate::spawn::converge_spawn`]
//! via [`handler::flows::spawn_flow`], and the frozen status/await seams —
//! so recovery answers are byte-identical to live answers by construction.
//!
//! Concurrency against live events (the plugin subscribes *before* the scan
//! runs, so new calls can arrive during it) is serialized by the
//! [`crate::reply::AnswerGuards`] claim set: the scan claims each call
//! outermost and calls the *unclaimed* flows directly — the same processing
//! code the claiming entry points delegate to, never re-claiming its own id
//! (plan note 1). The persisted [`reply::has_tool_result`] check stays as the
//! second line of defense across process generations.

use std::collections::HashMap;
use std::sync::Arc;

use rhd_chat_api::{ChatSummary, GetMessagesParams, ListChatsParams, ToolCall};
use rhd_chat_client::{ChatClient, ChatMonitor};

use crate::handler::{self, flows, HandlerCtx};
use crate::reply;
use crate::tags;

/// The three tool names whose unresolved calls recovery owns.
const SPAWN_TOOL: &str = "rhd_sub_chat";
const STATUS_TOOL: &str = "rhd_sub_chat_status";
const AWAIT_TOOL: &str = "rhd_sub_chat_await";

/// What one startup pass discovered and re-drove. Informational (logged);
/// the invariants themselves are enforced by the shared guards, not here.
#[derive(Debug, Default)]
pub struct RecoveryReport {
    /// Unresolved calls of our three names found across all chats.
    pub scanned_calls: usize,
    /// B was absent → full converge (create + seed + activate).
    pub respawned: usize,
    /// B existed still `paused` → queue completed and activated.
    pub reconciled_paused: usize,
    /// Live sync/await waiters re-registered on a still-running B.
    pub rewatched: usize,
    /// Immediate answers (status, await-completed, late async, errors).
    pub answered: usize,
    /// Calls whose persisted `tool` answer the scan found (skipped).
    pub skipped_answered: usize,
}

/// Fatal scan failure — nothing could be re-driven. Per-chat and per-call
/// failures are logged and skipped inside the scan (they are exactly the
/// states recovery must tolerate), and a partial pass is always safe:
/// everything is idempotent, so the next restart finishes the job.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("startup listChats for the subchat link index failed: {0}")]
    ListChats(String),
}

/// Link-tag lookup: `tool_call_id` → the subchat carrying its
/// `sub_chat:call:<id>` tag. Built once from a single unfiltered `listChats`
/// (implementation note 2): the scan touches every chat anyway, and the full
/// pass gives the duplicate-link diagnostic that per-call filtered queries
/// could not.
type SubChatIndex = HashMap<String, ChatSummary>;

/// Index the chat summaries by link tag. Pure so the pathological-state
/// handling (near-miss tags, duplicate links) is unit-testable without a
/// server.
fn index_chats(chats: Vec<ChatSummary>) -> SubChatIndex {
    let mut index = SubChatIndex::new();
    for summary in chats {
        for tag in &summary.tags {
            let Some(tool_call_id) = tags::parse_call_tag(tag) else {
                continue; // every other tag shape (incl. near-misses) is not ours
            };
            match index.get(tool_call_id) {
                // Duplicate link tags (pathological double-create): keep the
                // lowest chat id — creation order — and keep the diagnostic.
                Some(existing) if existing.id <= summary.id => {}
                Some(existing) => {
                    tracing::warn!(
                        tool_call_id,
                        kept_chat_id = existing.id,
                        ignored_chat_id = summary.id,
                        "duplicate sub_chat link tags; indexing the lowest chat id"
                    );
                    index.insert(tool_call_id.to_string(), summary.clone());
                }
                None => {
                    index.insert(tool_call_id.to_string(), summary.clone());
                }
            }
        }
    }
    index
}

/// Reconcile every unfinished `rhd_sub_chat*` tool call once at startup.
///
/// `client` is the connection used for the scan reads; it is expected to be
/// the very [`ChatClient`] behind `ctx` (what `plugin.rs` passes), so the
/// flows see identical state to the scan. Returns the report, or
/// [`RecoveryError`] when even the index read failed — meaning nothing was
/// examined at all.
pub async fn run(
    client: Arc<ChatClient>,
    chat_monitor: &ChatMonitor,
    ctx: HandlerCtx,
) -> Result<RecoveryReport, RecoveryError> {
    tracing::info!("sub_chat startup recovery started");
    let index = index_chats(
        client
            .list_chats(ListChatsParams { tags: vec![] })
            .await
            .map_err(|e| RecoveryError::ListChats(e.to_string()))?
            .chats,
    );

    let mut report = RecoveryReport::default();
    for chat_id in chat_monitor.get_chat_ids().await {
        // Note 4: a caller chat can vanish between the monitor snapshot and
        // this read — its parked calls could never be answered anyway; log
        // and move on (harmlessly rediscovered as absent next restart).
        let messages = match client
            .get_messages(GetMessagesParams {
                chat_id,
                with_unresolved_tool_calls: true,
                with_all_tags: vec![],
                with_any_tag: vec![],
            })
            .await
        {
            Ok(result) => result.messages,
            Err(e) => {
                tracing::warn!(
                    chat_id,
                    error = %e,
                    "recovery: cannot read chat history, skipping chat"
                );
                continue;
            }
        };

        // The server-computed filter returns exactly the messages whose
        // declared calls lack a persisted answer (note 3); our own answers
        // posted during this scan can never desync the loop because each
        // chat's list is re-read per iteration of the outer loop, and every
        // call re-checks `has_tool_result` before dispatch.
        for message in &messages {
            for tool_call in &message.tool_calls {
                let name = tool_call.function.name.as_str();
                if !matches!(name, SPAWN_TOOL | STATUS_TOOL | AWAIT_TOOL) {
                    continue; // foreign calls belong to other plugins
                }
                report.scanned_calls += 1;

                // Claim outermost, exactly once, then run the same
                // unclaimed flows the live entries delegate to (note 1).
                let Some(_claim) = ctx.guards.claim_guard(&tool_call.id) else {
                    tracing::debug!(
                        chat_id,
                        tool_call_id = %tool_call.id,
                        "recovery: call already being processed by a live handler, skipping"
                    );
                    continue;
                };
                match reply::has_tool_result(client.as_ref(), chat_id, &tool_call.id).await {
                    Ok(true) => {
                        tracing::debug!(
                            chat_id,
                            tool_call_id = %tool_call.id,
                            "recovery: call already answered, skipping"
                        );
                        report.skipped_answered += 1;
                        continue;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(
                            chat_id,
                            tool_call_id = %tool_call.id,
                            error = %e,
                            "recovery: duplicate-answer guard read failed, skipping call"
                        );
                        continue;
                    }
                }

                recover_call(&ctx, &index, chat_id, name, tool_call, &mut report).await;
            }
        }
    }

    tracing::info!(
        scanned_calls = report.scanned_calls,
        respawned = report.respawned,
        reconciled_paused = report.reconciled_paused,
        rewatched = report.rewatched,
        answered = report.answered,
        skipped_answered = report.skipped_answered,
        "sub_chat recovery finished"
    );
    Ok(report)
}

/// Re-drive one claimed, not-yet-answered call through the live-path flows
/// and fold the outcome into the report.
async fn recover_call(
    ctx: &HandlerCtx,
    index: &SubChatIndex,
    caller_chat_id: i64,
    name: &str,
    tool_call: &ToolCall,
    report: &mut RecoveryReport,
) {
    if name == SPAWN_TOOL {
        let flow = flows::spawn_flow(ctx, caller_chat_id, tool_call).await;
        if let flows::SpawnFlow::Async { sub_chat_id } | flows::SpawnFlow::Sync { sub_chat_id } =
            &flow
        {
            tracing::info!(
                caller_chat_id,
                tool_call_id = %tool_call.id,
                sub_chat_id,
                "recovery: unfinished spawn call re-converged"
            );
        }
        apply(spawn_classification(&flow, index.get(tool_call.id.as_str())), report);
        return;
    }

    // status / await: re-validate the target **fresh** — the world may have
    // changed (deletions, direct-child moves) while the plugin was down —
    // through the frozen processing code behind the Phase 5 seams.
    let tool = name;
    match handler::parse_target_chat_id(&tool_call.function.arguments) {
        Ok(target_chat_id) if name == STATUS_TOOL => {
            flows::status_flow(ctx, tool, caller_chat_id, &tool_call.id, target_chat_id).await;
            report.answered += 1;
        }
        Ok(target_chat_id) => {
            match flows::await_flow(ctx, tool, caller_chat_id, &tool_call.id, target_chat_id).await
            {
                flows::AwaitFlow::Answered => report.answered += 1,
                flows::AwaitFlow::Parked => report.rewatched += 1,
            }
        }
        Err(message) => {
            flows::answer_error(
                ctx,
                tool,
                caller_chat_id,
                &tool_call.id,
                &format!("invalid arguments: {message}"),
            )
            .await;
            report.answered += 1;
        }
    }
}

/// A single `RecoveryReport` bucket the classification helpers target.
#[derive(Debug, PartialEq, Eq)]
enum Counter {
    SkippedAnswered,
    Answered,
    Respawned,
    ReconciledPaused,
    Rewatched,
    /// Nothing was done (guard read failed) — no bucket.
    Untouched,
}

fn apply(counter: Counter, report: &mut RecoveryReport) {
    match counter {
        Counter::SkippedAnswered => report.skipped_answered += 1,
        Counter::Answered => report.answered += 1,
        Counter::Respawned => report.respawned += 1,
        Counter::ReconciledPaused => report.reconciled_paused += 1,
        Counter::Rewatched => report.rewatched += 1,
        Counter::Untouched => {}
    }
}

/// Plan classification, driven purely off the flow outcome and the index
/// snapshot taken at scan start: absent B → `respawned`; still-`paused` B →
/// `reconciled_paused`; active B → `rewatched` (sync) / `answered` (late
/// async notice). Answers that only *attempted* (immediate status/await,
/// error answers) count as `answered` too — the report is observational;
/// the guards, not the counts, own the exactly-once invariants.
///
/// Snapshot staleness is cosmetic: a live handler that fully processed the
/// same call between the index build and this claim leaves the call
/// answered (caught by `has_tool_result`) or parked (this pass then
/// re-parks the identical waiter; `answer_once` dedups the eventual drain).
fn spawn_classification(flow: &flows::SpawnFlow, linked: Option<&ChatSummary>) -> Counter {
    match flow {
        flows::SpawnFlow::AlreadyAnswered => Counter::SkippedAnswered,
        flows::SpawnFlow::GuardUnavailable => Counter::Untouched,
        flows::SpawnFlow::Rejected | flows::SpawnFlow::Failed => Counter::Answered,
        converged @ (flows::SpawnFlow::Async { .. } | flows::SpawnFlow::Sync { .. }) => {
            match linked {
                None => Counter::Respawned,
                Some(sub_chat) if tags::has_paused_tag(&sub_chat.tags) => Counter::ReconciledPaused,
                Some(_) if matches!(converged, flows::SpawnFlow::Sync { .. }) => {
                    Counter::Rewatched
                }
                Some(_) => Counter::Answered,
            }
        }
    }
}

#[cfg(test)]
mod tests;
