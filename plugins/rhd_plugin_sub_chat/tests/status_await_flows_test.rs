//! Phase 5 integration tests: the status/await **state flows** — fresh
//! status evaluation across message/queue edits and the await fast/slow
//! paths (plan tests 2–5).

mod common;

use common::{
    add_finished_message, add_queued_message, assert_answer, async_spawn, create_chat, drain_queue,
    drive_tool_call, setup, tool_answers, wait_for_answers,
};

/// Plan tests 2 + 5 (pending → completed flip, then full freshness): every
/// status call re-reads the chat, so answers flip back and forth with
/// message/queue edits; status itself never consults or feeds the watcher.
#[tokio::test]
async fn status_is_freshly_evaluated_on_every_call() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_b = async_spawn(&env, chat_a, "call_spawn_b", "status subject").await;

    // Pending right after activation: the seed queue is not drained.
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_st_1",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_st_1", "pending").await;

    // Drained queue but empty history is still not completed.
    drain_queue(&env.client, chat_b).await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_st_2",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_st_2", "pending").await;

    // Final finished assistant message → completed.
    add_finished_message(&env.client, chat_b, "assistant", "B FINAL").await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_st_3",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_st_3", "completed").await;

    // Freshness: a human queues new work → pending again (note 1).
    add_queued_message(&env.client, chat_b, "one more thing").await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_st_4",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_st_4", "pending").await;

    // …and back to completed once drained (last message still final).
    drain_queue(&env.client, chat_b).await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_st_5",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_st_5", "completed").await;

    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "status must never register waiters"
    );
}

/// Plan test 3 (await fast path): an already-completed direct child is
/// answered verbatim, synchronously, with no watcher registration at all.
#[tokio::test]
async fn await_fast_path_answers_completed_child_without_watcher() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_b = async_spawn(&env, chat_a, "call_spawn_fastb", "await subject").await;
    drain_queue(&env.client, chat_b).await;
    add_finished_message(&env.client, chat_b, "assistant", "AWAIT FAST ANSWER").await;

    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_await",
        "call_await_fast",
        &format!(r#"{{"chatId":{chat_b}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_await_fast", "AWAIT FAST ANSWER").await;
    assert!(
        !env.watcher.watched_ids().await.contains(&chat_b),
        "the fast path must not touch the watcher"
    );
}

/// Plan test 4 (await slow path + multi-waiter broadcast): awaits on a
/// pending child park unanswered; the completion answers every waiter with
/// the same verbatim content through the shared watcher machinery.
#[tokio::test]
async fn await_slow_path_parks_then_broadcasts_to_every_waiter() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_b = async_spawn(&env, chat_a, "call_spawn_slowb", "slow await subject").await;

    for call_id in ["call_await_s1", "call_await_s2"] {
        drive_tool_call(
            &env,
            chat_a,
            "rhd_sub_chat_await",
            call_id,
            &format!(r#"{{"chatId":{chat_b}}}"#),
        )
        .await;
        assert!(
            tool_answers(&env.client, chat_a, call_id).await.is_empty(),
            "{call_id} must stay parked while the subchat is pending"
        );
    }
    assert!(env.watcher.watched_ids().await.contains(&chat_b));

    // Completion: both waiters get the identical final content.
    drain_queue(&env.client, chat_b).await;
    add_finished_message(&env.client, chat_b, "assistant", "SHARED FINAL").await;
    wait_for_answers(&env.client, chat_a, &["call_await_s1", "call_await_s2"]).await;
    common::settle().await;
    assert_answer(&env.client, chat_a, "call_await_s1", "SHARED FINAL").await;
    assert_answer(&env.client, chat_a, "call_await_s2", "SHARED FINAL").await;
    assert!(
        !env.watcher.watched_ids().await.contains(&chat_b),
        "waiters are consumed by the drain"
    );
}
