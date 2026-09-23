//! Phase 7 e2e, plan scenarios 4–6: nesting lineage, the direct-child scope
//! rule, and spawn validation — all driven from the MODEL's seat (tool calls
//! emitted by the mock, answered by the real plugin pipeline).
//!
//! Scenario 4: a sync-spawned B sync-spawns C; the answer flows C → B → A
//! through two nested parked waits, and C's `root:` tag is INHERITED from A,
//! not B. Scenario 5: status/await on a grandchild from the root are rejected
//! with the byte-exact wire error strings and A's loop keeps living.
//! Scenario 6: invalid spawn arguments are answered with wire-exact errors
//! and create zero chats.

mod common;

use std::time::Duration;

use rhd_plugin_sub_chat::tags;
use tokio::time::timeout;

use common::full_env::FullEnv;
use common::polling::{wait_answers, wait_final_assistant, wait_subchat};
use common::routing::{self, spawn_arguments, streamed_tool_call, target_arguments};
use common::{production_tags, settle, tool_answers};

/// Plan scenario 4: B (sync-spawned by A) sync-spawns C. Asserts the full
/// two-level parking chain resolves in order, and C's tag set is
/// `[parent:B, root:A, sub_chat:call:<tc>]` — the root is INHERITED through
/// B's lineage, never re-pointed at B.
#[tokio::test]
async fn e2e_nested_sync_spawn_inherits_root() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        let marker_parent = "E2E-PARENT-S4";
        let marker_b = "E2E-TASK-B4";
        let marker_c = "E2E-TASK-C4";
        env.listener.register(
            marker_parent,
            vec![
                streamed_tool_call(
                    "call_s4_b",
                    "rhd_sub_chat",
                    &spawn_arguments(&[format!("{marker_b} outer task")], false),
                ),
                routing::text_response("PARENT DONE S4"),
            ],
        );
        env.listener.register(
            marker_b,
            vec![
                streamed_tool_call(
                    "call_s4_c",
                    "rhd_sub_chat",
                    &spawn_arguments(&[format!("{marker_c} inner task")], false),
                ),
                routing::text_response("B ANSWER S4"),
            ],
        );
        env.listener
            .register(marker_c, vec![routing::text_response("C ANSWER S4")]);

        let a = env.create_chat("e2e parent S4").await;
        env.wait_tools_registered(a).await;
        env.queue_user(a, &format!("start {marker_parent}")).await;

        let b = wait_subchat(&env, "call_s4_b").await;
        let c = wait_subchat(&env, "call_s4_c").await;

        // C's lineage: direct child of B, ROOT of A (inherited).
        let mut expected_c = vec![
            tags::parent_tag(b),
            tags::root_tag(a),
            tags::call_tag("call_s4_c"),
        ];
        expected_c.sort();
        assert_eq!(
            production_tags(&env.chat_tags(c).await),
            expected_c,
            "C carries parent:B + INHERITED root:A + link"
        );
        assert!(
            !env.chat_tags(c).await.contains(&tags::root_tag(b)),
            "root must be inherited, not re-pointed at B"
        );
        let mut expected_b = vec![
            tags::parent_tag(a),
            tags::root_tag(a),
            tags::call_tag("call_s4_b"),
        ];
        expected_b.sort();
        assert_eq!(production_tags(&env.chat_tags(b).await), expected_b);

        // Two nested waits: C's answer lands in B, B's completion in A.
        wait_answers(&env, b, &["call_s4_c"]).await;
        let c_answers = tool_answers(&env.client, b, "call_s4_c").await;
        assert_eq!(c_answers.len(), 1);
        assert_eq!(c_answers[0].content, "C ANSWER S4");
        wait_answers(&env, a, &["call_s4_b"]).await;
        let b_answers = tool_answers(&env.client, a, "call_s4_b").await;
        assert_eq!(b_answers.len(), 1);
        assert_eq!(b_answers[0].content, "B ANSWER S4");
        // A's tool answer is byte-identical to B's final assistant content —
        // B only completed after C's answer flowed in.
        wait_final_assistant(&env, b, "B ANSWER S4").await;
        assert_eq!(b_answers[0].content, "B ANSWER S4");
        wait_final_assistant(&env, a, "PARENT DONE S4").await;

        assert_eq!(env.listener.count_for(marker_parent), 2);
        assert_eq!(env.listener.count_for(marker_b), 2);
        assert_eq!(env.listener.count_for(marker_c), 1);
        env.shutdown();
    })
    .await
    .expect("scenario 4 timed out");
}

/// Plan scenario 5: from the model's seat, status/await on a GRANDCHILD are
/// rejected with the exact `is not a direct subchat` strings (AD-5), and the
/// caller's loop demonstrably continues after both error answers.
#[tokio::test]
async fn e2e_scope_enforcement_rejects_grandchild() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        let marker_parent = "E2E-PARENT-S5";
        let marker_b = "E2E-TASK-B5";
        let marker_c = "E2E-TASK-C5";
        env.listener.register(
            marker_parent,
            vec![streamed_tool_call(
                "call_s5_b",
                "rhd_sub_chat",
                &spawn_arguments(&[format!("{marker_b} work")], true),
            )],
        );
        env.listener.register(
            marker_b,
            vec![
                streamed_tool_call(
                    "call_s5_c",
                    "rhd_sub_chat",
                    &spawn_arguments(&[format!("{marker_c} deep work")], true),
                ),
                routing::text_response("B DONE S5"),
            ],
        );
        env.listener
            .register(marker_c, vec![routing::text_response("C DONE S5")]);

        let a = env.create_chat("e2e parent S5").await;
        env.wait_tools_registered(a).await;
        env.queue_user(a, &format!("start {marker_parent}")).await;
        let b = wait_subchat(&env, "call_s5_b").await;
        wait_final_assistant(&env, b, "B DONE S5").await;
        let c = wait_subchat(&env, "call_s5_c").await;

        // A reaches past B at C with both tools — rejected, byte-exact.
        env.listener.register(
            marker_parent,
            vec![
                streamed_tool_call("call_s5_status", "rhd_sub_chat_status", &target_arguments(c)),
                streamed_tool_call("call_s5_await", "rhd_sub_chat_await", &target_arguments(c)),
                routing::text_response("PARENT DONE S5"),
            ],
        );
        wait_answers(&env, a, &["call_s5_status", "call_s5_await"]).await;
        let status = tool_answers(&env.client, a, "call_s5_status").await;
        assert_eq!(status.len(), 1);
        assert_eq!(
            status[0].content,
            format!("rhd_sub_chat_status error: chat {c} is not a direct subchat of this chat")
        );
        let awaited = tool_answers(&env.client, a, "call_s5_await").await;
        assert_eq!(awaited.len(), 1);
        assert_eq!(
            awaited[0].content,
            format!("rhd_sub_chat_await error: chat {c} is not a direct subchat of this chat")
        );

        // The loop demonstrably continued past both error answers.
        wait_final_assistant(&env, a, "PARENT DONE S5").await;
        // The rejected calls touched nothing on C: no tool messages ever,
        // history still seed → final answer.
        settle().await;
        let c_messages = env.messages(c).await;
        assert!(
            c_messages.iter().all(|m| m.role != "tool"),
            "A's rejected status/await must not answer anything on C"
        );
        assert_eq!(c_messages.last().map(|m| m.content.as_str()), Some("C DONE S5"));
        assert_eq!(env.listener.count_for(marker_parent), 4);
        env.shutdown();
    })
    .await
    .expect("scenario 5 timed out");
}

/// Plan scenario 6: invalid spawn arguments from the model's seat answer the
/// call with the wire-exact validation error and create NO new chat (the
/// `list_chats` delta across both bad calls is zero).
#[tokio::test]
async fn e2e_validation_errors_create_no_chats() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        let marker_parent = "E2E-PARENT-S6";
        env.listener.register(
            marker_parent,
            vec![
                // Role outside {system, user}.
                streamed_tool_call(
                    "call_s6_badrole",
                    "rhd_sub_chat",
                    r#"{"messages":[{"role":"assistant","content":"E2E-BADROLE-S6"}]}"#,
                ),
                // Reserved tag prefix in user-supplied tags.
                streamed_tool_call(
                    "call_s6_badtag",
                    "rhd_sub_chat",
                    r#"{"messages":[{"role":"user","content":"E2E-BADTAG-S6"}],"tags":["parent:99"]}"#,
                ),
                routing::text_response("PARENT DONE S6"),
            ],
        );

        let a = env.create_chat("e2e parent S6").await;
        env.wait_tools_registered(a).await;
        let chats_before = env
            .client
            .list_chats(rhd_chat_api::ListChatsParams { tags: vec![] })
            .await
            .expect("listChats failed")
            .chats
            .into_iter()
            .map(|c| c.id)
            .collect::<Vec<_>>();
        env.queue_user(a, &format!("start {marker_parent}")).await;

        wait_answers(&env, a, &["call_s6_badrole", "call_s6_badtag"]).await;
        let badrole = tool_answers(&env.client, a, "call_s6_badrole").await;
        assert_eq!(badrole.len(), 1);
        assert_eq!(
            badrole[0].content,
            "rhd_sub_chat error: messages[0]: role 'assistant' is not allowed (only 'system' and 'user')"
        );
        let badtag = tool_answers(&env.client, a, "call_s6_badtag").await;
        assert_eq!(badtag.len(), 1);
        assert_eq!(
            badtag[0].content,
            "rhd_sub_chat error: invalid tags: tag 'parent:99' uses a reserved prefix"
        );

        // ZERO chats spawned across both validation failures.
        settle().await;
        let chats_after = env
            .client
            .list_chats(rhd_chat_api::ListChatsParams { tags: vec![] })
            .await
            .expect("listChats failed")
            .chats
            .into_iter()
            .map(|c| c.id)
            .collect::<Vec<_>>();
        assert_eq!(
            chats_after, chats_before,
            "validation errors must not create chats"
        );
        wait_final_assistant(&env, a, "PARENT DONE S6").await;
        // Exactly three provider round trips on A's lane, nothing anywhere else.
        assert_eq!(env.listener.count_for(marker_parent), 3);
        let linked = env
            .client
            .list_chats(rhd_chat_api::ListChatsParams { tags: vec![] })
            .await
            .expect("listChats failed")
            .chats
            .into_iter()
            .filter(|c| c.tags.iter().any(|t| t.starts_with("sub_chat:")))
            .count();
        assert_eq!(linked, 0, "validation errors must not create subchats");
        env.shutdown();
    })
    .await
    .expect("scenario 6 timed out");
}
