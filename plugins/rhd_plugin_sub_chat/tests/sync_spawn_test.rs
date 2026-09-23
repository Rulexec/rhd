//! Integration tests: the Phase 4 sync answer path — completion watcher.
//!
//! Chat server + this plugin's code only — no ai_completions, no mock AI.
//! Subchat completion is driven synthetically: spawn via `handle_spawn`
//! (async=false), clear the seed queue (mirroring what `ai_completions`
//! drains in production), then append messages so the `ChatMonitor` hook
//! evaluates the predicate on real state-change events. Shared harness from
//! `tests/common`; the sync-spawn fixtures specific to this path from
//! `common::sync_spawn`. The duplicate-event and multi-waiter broadcast
//! scenarios live in `sync_spawn_broadcast_test.rs`.

mod common;

use common::sync_spawn::{sync_spawn, wait_monitor_state};
use common::{
    add_finished_message, create_chat, drain_queue, settle, setup, tool_answers, wait_for_answers,
};

/// Plan scenarios 1–3: the sync call parks after spawn; intermediate states
/// (no messages, last=user) keep it unanswered; the first finished assistant
/// message answers it byte-identically and consumes the waiter.
#[tokio::test]
async fn sync_spawn_parks_then_completion_answers_verbatim() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;

    // Park: after converge + register, A's call must stay unanswered.
    let chat_b = sync_spawn(&env, chat_a, "call_sync_1", "sync task").await;
    assert!(
        env.watcher.watched_ids().await.contains(&chat_b),
        "the sync call must register a waiter on B"
    );
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "scenario 1: sync call stays unanswered right after spawn"
    );

    // Negative state 1 (plan step 2's precondition): queue drained, but no
    // messages at all → predicate false.
    drain_queue(&env.client, chat_b).await;
    wait_monitor_state(&env.monitor, chat_b, "empty queue", |s| {
        s.queued_messages_count == 0 && s.messages.is_empty()
    })
    .await;
    settle().await;
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "empty history must not complete the subchat"
    );

    // Negative state 2 (plan scenario 3): last message is user → predicate false.
    add_finished_message(&env.client, chat_b, "user", "intermediate").await;
    wait_monitor_state(&env.monitor, chat_b, "user message last", |s| {
        s.messages.last().is_some_and(|m| m.role == "user")
    })
    .await;
    settle().await;
    assert!(
        tool_answers(&env.client, chat_a, "call_sync_1")
            .await
            .is_empty(),
        "a trailing user message must not complete the subchat"
    );

    // Plan scenario 2: finished assistant message → hook fires → A answered
    // with the content byte-identical.
    add_finished_message(&env.client, chat_b, "assistant", "SUBCHAT FINAL ANSWER").await;
    wait_for_answers(&env.client, chat_a, &["call_sync_1"]).await;
    let answers = tool_answers(&env.client, chat_a, "call_sync_1").await;
    assert_eq!(answers.len(), 1, "exactly one answer");
    assert_eq!(answers[0].role, "tool");
    assert_eq!(answers[0].content, "SUBCHAT FINAL ANSWER");
    assert!(
        !env.watcher.watched_ids().await.contains(&chat_b),
        "waiters must be consumed by the drain (take-before-answer)"
    );
}
