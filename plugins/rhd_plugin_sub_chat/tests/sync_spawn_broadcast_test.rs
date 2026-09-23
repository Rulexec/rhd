//! Integration tests: the Phase 4 sync answer path — broadcast safety.
//!
//! Companion of `sync_spawn_test.rs` (same synthetic completion setup,
//! shared `tests/common` harness and `common::sync_spawn` fixtures): the
//! watcher must collapse a burst of completion-bearing state-change events
//! into exactly one answer (atomic take + `AnswerGuards`), and every waiter
//! on one subchat must receive the same content — including a waiter that
//! registers after the completion.

mod common;

use rhd_plugin_sub_chat::watcher::Waiter;

use common::sync_spawn::{sync_spawn, toggle_tags};
use common::{
    add_finished_message, create_chat, drain_queue, settle, setup, tool_answers, wait_for_answers,
};

/// Plan scenario 4: a burst of completion-bearing state-change events (the
/// final message plus two tag add/remove cycles) yields exactly one answer —
/// atomic take + AnswerGuards kill the duplicates.
#[tokio::test]
async fn duplicate_completion_events_answer_exactly_once() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_b = sync_spawn(&env, chat_a, "call_dup_events", "dup task").await;
    assert!(
        tool_answers(&env.client, chat_a, "call_dup_events")
            .await
            .is_empty(),
        "still parked after spawn"
    );
    drain_queue(&env.client, chat_b).await;

    // Complete B, then hammer it with tag toggles: every event carries a
    // completed state, all drain attempts race on the atomic take.
    add_finished_message(&env.client, chat_b, "assistant", "DUP FINAL ANSWER").await;
    toggle_tags(&env.client, chat_b, "watcher:toggle1").await;
    toggle_tags(&env.client, chat_b, "watcher:toggle2").await;

    wait_for_answers(&env.client, chat_a, &["call_dup_events"]).await;
    settle().await; // let any straggler drain run before counting
    let answers = tool_answers(&env.client, chat_a, "call_dup_events").await;
    assert_eq!(
        answers.len(),
        1,
        "duplicate completion events must produce exactly one answer, got {:?}",
        answers.iter().map(|a| &a.content).collect::<Vec<_>>()
    );
    assert_eq!(answers[0].content, "DUP FINAL ANSWER");
}

/// Plan scenario 5 (+ impl note 1): several waiters on one subchat all get
/// the same content from a single completion; a waiter registered after the
/// completion is answered immediately by `register_or_complete` itself
/// (the register-then-check race closer).
#[tokio::test]
async fn multiple_waiters_broadcast_and_late_registration_answers_immediately() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_b = sync_spawn(&env, chat_a, "call_w1", "broadcast task").await;
    drain_queue(&env.client, chat_b).await;

    // Second waiter on the same, still-incomplete subchat (simulates a
    // concurrent sync spawn / a Phase 5 `rhd_sub_chat_await`).
    env.watcher
        .register_or_complete(
            chat_b,
            Waiter {
                parent_chat_id: chat_a,
                tool_call_id: "call_w2".to_string(),
            },
        )
        .await;
    assert!(
        tool_answers(&env.client, chat_a, "call_w2")
            .await
            .is_empty(),
        "registering on an incomplete subchat must not answer"
    );

    // Completion broadcasts to both waiters with identical content.
    add_finished_message(&env.client, chat_b, "assistant", "SHARED ANSWER").await;
    wait_for_answers(&env.client, chat_a, &["call_w1", "call_w2"]).await;
    settle().await;
    let first = tool_answers(&env.client, chat_a, "call_w1").await;
    let second = tool_answers(&env.client, chat_a, "call_w2").await;
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].content, "SHARED ANSWER");
    assert_eq!(
        first[0].content, second[0].content,
        "both waiters get the same content"
    );

    // Late registration on an already-completed subchat: answered
    // immediately by register_or_complete's own check, with no new event on
    // B ever firing.
    env.watcher
        .register_or_complete(
            chat_b,
            Waiter {
                parent_chat_id: chat_a,
                tool_call_id: "call_w3".to_string(),
            },
        )
        .await;
    wait_for_answers(&env.client, chat_a, &["call_w3"]).await;
    let late = tool_answers(&env.client, chat_a, "call_w3").await;
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].content, "SHARED ANSWER");
    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "all waiters consumed"
    );
}
