//! Phase 5 integration tests: the status/await **gate** — the direct-child
//! rule (AD-5) and the Wire-Contract error answers that always resolve the
//! caller's loop (plan tests 1 + 6).

mod common;

use rhd_plugin_sub_chat::tags;

use common::{
    assert_answer, async_spawn, create_chat, drive_tool_call, get_chat, setup, tool_answers,
};

/// Plan test 1 (direct-child rule) + test 6 (every rejection is answered):
/// a real child works; a child of another chat, a grandchild, the caller
/// itself, and an unknown id all get the Wire-Contract error — for both
/// tools — and nothing ever parks.
#[tokio::test]
async fn direct_child_rule_gates_status_and_await() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let chat_o = create_chat(&env.client, "other parent", vec![]).await;

    let chat_b1 = async_spawn(&env, chat_a, "call_spawn_b1", "real child").await;
    assert!(tags::is_direct_child(
        &get_chat(&env.client, chat_b1).await.chat.tags,
        chat_a
    ));
    // Foreign child: created directly with another parent's tag.
    let chat_b2 = create_chat(
        &env.client,
        "foreign",
        vec![tags::parent_tag(chat_o), tags::root_tag(chat_o)],
    )
    .await;
    // Grandchild: spawned from B1, so it is NOT a direct child of A.
    let chat_c = async_spawn(&env, chat_b1, "call_spawn_c", "nested").await;

    let not_child = |tool: &str, target: i64| {
        format!("{tool} error: chat {target} is not a direct subchat of this chat")
    };

    // Real child works: status answers the fresh pending state (seed queue).
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_status_b1",
        &format!(r#"{{"chatId":{chat_b1}}}"#),
    )
    .await;
    assert_answer(&env.client, chat_a, "call_status_b1", "pending").await;

    // Foreign child: both tools reject with the direct-child error.
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_status_b2",
        &format!(r#"{{"chatId":{chat_b2}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_status_b2",
        &not_child("rhd_sub_chat_status", chat_b2),
    )
    .await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_await",
        "call_await_b2",
        &format!(r#"{{"chatId":{chat_b2}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_await_b2",
        &not_child("rhd_sub_chat_await", chat_b2),
    )
    .await;

    // Grandchild denial and self-await denial (A carries no `parent:A`).
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_status_c",
        &format!(r#"{{"chatId":{chat_c}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_status_c",
        &not_child("rhd_sub_chat_status", chat_c),
    )
    .await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_await",
        "call_await_self",
        &format!(r#"{{"chatId":{chat_a}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_await_self",
        &not_child("rhd_sub_chat_await", chat_a),
    )
    .await;

    // No rejection ever reached the watcher: every rejected call was answered
    // at the error site, so nothing parked the caller's loop.
    common::settle().await;
    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "rejections must never register waiters"
    );
}

/// Plan test 6, unknown-target + malformed-argument half: every rejected
/// call shape produces its error answer, so the caller's loop never parks.
#[tokio::test]
async fn unknown_targets_and_invalid_arguments_are_answered() {
    let env = setup().await;
    let chat_a = create_chat(&env.client, "caller", vec![]).await;
    let missing = 999_999_i64;

    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_status",
        "call_status_missing",
        &format!(r#"{{"chatId":{missing}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_status_missing",
        &format!("rhd_sub_chat_status error: chat {missing} not found"),
    )
    .await;
    drive_tool_call(
        &env,
        chat_a,
        "rhd_sub_chat_await",
        "call_await_missing",
        &format!(r#"{{"chatId":{missing}}}"#),
    )
    .await;
    assert_answer(
        &env.client,
        chat_a,
        "call_await_missing",
        &format!("rhd_sub_chat_await error: chat {missing} not found"),
    )
    .await;

    // Malformed arguments are answered before any lookup happens.
    let cases: [(&str, &str, &str); 4] = [
        ("rhd_sub_chat_status", "call_sa_str", r#"{"chatId":"7"}"#),
        ("rhd_sub_chat_status", "call_sa_empty", "{}"),
        ("rhd_sub_chat_await", "call_aa_garbage", "not json"),
        (
            "rhd_sub_chat_await",
            "call_aa_array",
            "[{\"chatId\":7}]",
        ),
    ];
    for (name, call_id, arguments) in cases {
        drive_tool_call(&env, chat_a, name, call_id, arguments).await;
        let answers = tool_answers(&env.client, chat_a, call_id).await;
        assert_eq!(answers.len(), 1, "{call_id}: exactly one answer");
        assert_eq!(answers[0].role, "tool");
        assert!(
            answers[0]
                .content
                .starts_with(&format!("{name} error: invalid arguments: ")),
            "unexpected answer: {:?}",
            answers[0].content
        );
        assert!(
            !answers[0].content.contains('\n'),
            "answer must be single-line"
        );
    }

    common::settle().await;
    assert!(
        env.watcher.watched_ids().await.is_empty(),
        "rejected calls must never register waiters"
    );
}
