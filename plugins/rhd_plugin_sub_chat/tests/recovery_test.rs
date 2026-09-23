//! Integration tests: Phase 6 startup recovery.
//!
//! Chat server + this plugin's code only (per the plan): no ai_completions,
//! no mock AI, no real process kill — every crash state the scan can meet on
//! startup is constructed directly in stored history via
//! [`common::crash_states`], then `recovery::run` is driven through the exact
//! `plugin.rs` call shape. Answers must arrive (or stay parked) with the
//! same content the live path would have produced, exactly once.

mod common;

use rhd_chat_api::ListChatsParams;
use rhd_plugin_sub_chat::{spawn, tags};

use common::crash_states::{
    add_assistant_calls, answer_call, chat_ids, message_count, queue_contents, run_recovery,
    tool_call, wait_monitor_sees,
};
use common::{
    add_finished_message, add_queued_message, assert_answer, create_chat, drain_queue, get_chat,
    settle, setup, tool_answers, wait_for_answers, TestEnv,
};

fn q(role: &str, content: &str) -> (String, String) {
    (role.to_string(), content.to_string())
}

/// The full AD-4 tag set a converged subchat would carry, `paused` included.
fn sub_tags(a: i64, call_id: &str, paused: bool) -> Vec<String> {
    let mut tags = vec![
        tags::parent_tag(a),
        tags::root_tag(a),
        tags::call_tag(call_id),
    ];
    if paused {
        tags.push(tags::PAUSED_TAG.to_string());
    }
    tags
}

/// Resolve the single subchat linked to `tool_call_id` by its link tag.
async fn linked_subchat(env: &TestEnv, tool_call_id: &str) -> i64 {
    let link = tags::call_tag(tool_call_id);
    let matches: Vec<_> = env
        .client
        .list_chats(ListChatsParams {
            tags: vec![link.clone()],
        })
        .await
        .expect("listChats failed")
        .chats
        .into_iter()
        .filter(|c| c.tags.iter().any(|t| t == &link))
        .collect();
    assert_eq!(matches.len(), 1, "exactly one chat linked to {tool_call_id}");
    matches[0].id
}

/// Plan scenario 1: paused B with only the first of two planned messages —
/// recovery enqueues message #2 in order, activates (removes `paused` last),
/// leaves A's sync call parked on a registered waiter, and counts it as
/// `reconciled_paused`.
#[tokio::test]
async fn paused_partial_queue_is_reconciled_in_order_and_activated() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(&env.client, "stale sub", sub_tags(a, "tc_paused", true)).await;
    add_queued_message(&env.client, b, "seed one").await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_paused",
            "rhd_sub_chat",
            r#"{"messages":[{"role":"user","content":"seed one"},{"role":"user","content":"seed two"}]}"#,
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;
    wait_monitor_sees(&env.monitor, b).await;

    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1);
    assert_eq!(report.reconciled_paused, 1, "paused B → reconciled bucket");
    assert_eq!(report.respawned, 0);
    assert_eq!(report.answered, 0);
    assert_eq!(report.skipped_answered, 0);
    assert_eq!(
        queue_contents(&env.client, b).await,
        vec![q("user", "seed one"), q("user", "seed two")],
        "missing suffix enqueued in plan order, nothing duplicated"
    );
    assert!(
        !tags::has_paused_tag(&get_chat(&env.client, b).await.chat.tags),
        "activation is the last step of a complete converge"
    );

    // Sync plan: A's call stays unanswered and parks on a real waiter.
    settle().await;
    assert!(
        tool_answers(&env.client, a, "tc_paused").await.is_empty(),
        "the scan must not answer a sync spawn"
    );
    assert!(
        env.watcher.watched_ids().await.contains(&b),
        "recovery must re-register the parked waiter"
    );
}

/// Plan scenario 2: unresolved sync call, B absent — recovery performs the
/// full converge (exactly one new chat, tagged, seeded, activated, watched),
/// and the eventual live completion answers A verbatim through the watcher.
#[tokio::test]
async fn absent_subchat_is_full_respawn_and_final_answer_reaches_caller() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_absent",
            "rhd_sub_chat",
            r#"{"messages":[{"role":"system","content":"ctx"},{"role":"user","content":"full task"}]}"#,
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;

    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1);
    assert_eq!(report.respawned, 1, "index had no B → full converge");
    assert_eq!(report.reconciled_paused, 0);
    let b = linked_subchat(&env, "tc_absent").await;
    let sub = get_chat(&env.client, b).await;
    assert!(
        tags::is_direct_child(&sub.chat.tags, a),
        "parent link present from the atomic create"
    );
    assert!(!tags::has_paused_tag(&sub.chat.tags));
    assert_eq!(
        queue_contents(&env.client, b).await,
        vec![q("system", "ctx"), q("user", "full task")],
        "full seed queue rebuilt in order"
    );
    assert!(env.watcher.watched_ids().await.contains(&b));

    // The respawned chat behaves like a live sync spawn from here on:
    // completing it answers A with the final content verbatim, once.
    wait_monitor_sees(&env.monitor, b).await;
    drain_queue(&env.client, b).await;
    add_finished_message(&env.client, b, "assistant", "REBORN ANSWER").await;
    wait_for_answers(&env.client, a, &["tc_absent"]).await;
    settle().await;
    assert_answer(&env.client, a, "tc_absent", "REBORN ANSWER").await;
}

/// Plan scenario 3: async call, crash before the notice, B already active
/// and completed — the late notice answers the exact golden string once.
#[tokio::test]
async fn late_async_notice_is_answered_exactly_once() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(
        &env.client,
        "ran while plugin was down",
        sub_tags(a, "tc_late", false),
    )
    .await;
    add_finished_message(&env.client, b, "assistant", "done long ago").await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_late",
            "rhd_sub_chat",
            r#"{"messages":[{"role":"user","content":"x"}],"async":true}"#,
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;
    wait_monitor_sees(&env.monitor, b).await;

    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1);
    assert_eq!(report.answered, 1, "active B + async → late notice");
    assert_eq!(report.respawned, 0);
    assert_eq!(report.rewatched, 0);
    settle().await;
    assert_answer(&env.client, a, "tc_late", &spawn::async_notice(b)).await;
    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "async recovery registers no waiter"
    );
}

/// Plan scenario 4: the call already has its persisted answer — the scan
/// (which only sees it because a foreign call in the same message is still
/// unresolved) skips it with `skipped_answered == 1` and posts nothing new.
#[tokio::test]
async fn already_answered_call_is_untouched() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(&env.client, "completed sub", sub_tags(a, "tc_done", false)).await;
    add_finished_message(&env.client, b, "assistant", "old final").await;
    add_assistant_calls(
        &env.client,
        a,
        &[
            tool_call(
                "tc_done",
                "rhd_sub_chat",
                r#"{"messages":[{"role":"user","content":"y"}],"async":true}"#,
            ),
            // Keeps the message visible to the unresolved filter; recovery
            // must never touch foreign names.
            tool_call("call_foreign", "web_search", "{}"),
        ],
    )
    .await;
    answer_call(&env.client, a, "tc_done", "ANSWERED LONG AGO").await;

    let chats_before = chat_ids(&env.client).await.len();
    let messages_before = message_count(&env.client, a).await;
    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1, "only our call counts");
    assert_eq!(report.skipped_answered, 1);
    assert_eq!(report.respawned, 0);
    assert_eq!(report.reconciled_paused, 0);
    assert_eq!(report.answered, 0);
    settle().await;
    assert_eq!(chat_ids(&env.client).await.len(), chats_before, "no new chats");
    assert_eq!(
        message_count(&env.client, a).await,
        messages_before,
        "no duplicate answer message"
    );
    let answers = tool_answers(&env.client, a, "tc_done").await;
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].content, "ANSWERED LONG AGO", "original kept");
    assert!(env.watcher.watched_ids().await.is_empty());
}

/// Plan scenario 5: unresolved `rhd_sub_chat_await` on a completed B — the
/// frozen await seam answers the final content immediately (fresh
/// direct-child revalidation), no watcher round-trip.
#[tokio::test]
async fn await_across_restart_is_answered_immediately() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(
        &env.client,
        "child finished while down",
        vec![tags::parent_tag(a), tags::root_tag(a)],
    )
    .await;
    add_finished_message(&env.client, b, "assistant", "FINAL AFTER RESTART").await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_await",
            "rhd_sub_chat_await",
            &format!(r#"{{"chatId":{b}}}"#),
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;

    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1);
    assert_eq!(report.answered, 1, "completed target → immediate answer");
    assert_eq!(report.rewatched, 0);
    settle().await;
    assert_answer(&env.client, a, "tc_await", "FINAL AFTER RESTART").await;
    assert!(env.watcher.watched_ids().await.is_empty());
}

/// Plan scenario 6: human tampering with the paused queue — refuse to
/// guess (AD-13): the exact mid-spawn error answer lands in A, B stays
/// paused, its foreign queue content untouched.
#[tokio::test]
async fn tampered_paused_queue_refuses_reconcile_and_stays_paused() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(&env.client, "tampered sub", sub_tags(a, "tc_tamper", true)).await;
    add_queued_message(&env.client, b, "HUMAN INJECTED").await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_tamper",
            "rhd_sub_chat",
            r#"{"messages":[{"role":"user","content":"planned seed"}]}"#,
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;
    wait_monitor_sees(&env.monitor, b).await;

    let report = run_recovery(&env).await;

    assert_eq!(report.scanned_calls, 1);
    assert_eq!(report.answered, 1, "refusal is still an answer");
    assert_eq!(report.respawned, 0);
    assert_eq!(report.reconciled_paused, 0);
    // Pinned verbatim against the live path's wording — including the
    // pre-existing (since Phase 3) double `rhd_sub_chat error: ` prefix,
    // because `prepare_failure_answer` embeds it and `answer_error` adds it
    // again. Recovery REUSES that exact code, so an honest recovery test
    // must expect it; normalizing the text is out of scope here.
    assert_answer(
        &env.client,
        a,
        "tc_tamper",
        &format!(
            "rhd_sub_chat error: rhd_sub_chat error: failed to prepare subchat {b}: queued starting messages no longer match the tool call arguments (subchat left paused for recovery)"
        ),
    )
    .await;
    let sub = get_chat(&env.client, b).await;
    assert!(
        tags::has_paused_tag(&sub.chat.tags),
        "a refused reconciliation must NOT activate B"
    );
    assert_eq!(
        queue_contents(&env.client, b).await,
        vec![q("user", "HUMAN INJECTED")],
        "foreign queue content left for the operator"
    );
}

/// Plan scenario 7: two back-to-back passes change nothing — the second
/// re-scans the still-parked sync call, re-converges (no-op on an active
/// B), and adds zero chats, messages and answers.
#[tokio::test]
async fn double_run_is_idempotent() {
    let env = setup().await;
    let a = create_chat(&env.client, "caller", vec![]).await;
    let b = create_chat(&env.client, "partial sub", sub_tags(a, "tc_double", true)).await;
    add_queued_message(&env.client, b, "seed one").await;
    add_assistant_calls(
        &env.client,
        a,
        &[tool_call(
            "tc_double",
            "rhd_sub_chat",
            r#"{"messages":[{"role":"user","content":"seed one"},{"role":"user","content":"seed two"}]}"#,
        )],
    )
    .await;
    wait_monitor_sees(&env.monitor, a).await;
    wait_monitor_sees(&env.monitor, b).await;

    let first = run_recovery(&env).await;
    assert_eq!(first.reconciled_paused, 1);

    let chats_after = chat_ids(&env.client).await.len();
    let a_messages_after = message_count(&env.client, a).await;
    let b_messages_after = message_count(&env.client, b).await;
    let b_queue_after = queue_contents(&env.client, b).await;

    let second = run_recovery(&env).await;

    assert_eq!(second.scanned_calls, 1, "the sync call is still unresolved");
    assert_eq!(second.respawned, 0, "B now exists — nothing to create");
    assert_eq!(second.reconciled_paused, 0, "and it is no longer paused");
    assert_eq!(
        second.rewatched, 1,
        "re-parked as active-sync (duplicate waiter is deduped by the answer guards)"
    );
    assert_eq!(second.answered, 0);
    assert_eq!(second.skipped_answered, 0);
    settle().await;
    assert_eq!(chat_ids(&env.client).await.len(), chats_after, "no new chats");
    assert_eq!(
        message_count(&env.client, a).await,
        a_messages_after,
        "no new caller messages"
    );
    assert_eq!(
        message_count(&env.client, b).await,
        b_messages_after,
        "no new subchat messages"
    );
    assert_eq!(
        queue_contents(&env.client, b).await,
        b_queue_after,
        "seed queue not duplicated"
    );
    assert!(
        tool_answers(&env.client, a, "tc_double").await.is_empty(),
        "still parked, still unanswered"
    );
}
