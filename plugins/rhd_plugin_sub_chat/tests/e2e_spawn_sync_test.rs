//! Phase 7 e2e, plan scenarios 1–3: spawn flows through the FULL pipeline
//! (chat server + ai_completions + sub_chat + one content-routed mock AI).
//!
//! Scenario 1 is the flagship sync round trip; scenario 2 walks the async +
//! status + await ladder; scenario 3 parks a subchat on a provider error and
//! recovers it via the operator flow. All responses are streamed shapes —
//! see [`common::routing`] for the matcher discipline.

mod common;

use std::time::Duration;

use rhd_mock_ai_provider::rhd_ai_client::ChatMessage;
use rhd_plugin_sub_chat::tags;
use tokio::time::timeout;

use common::full_env::FullEnv;
use common::polling::{
    find_subchat, wait_answers, wait_assistant_call, wait_chat_tag, wait_for_count,
    wait_final_assistant, wait_subchat,
};
use common::routing::{
    self, provider_error, spawn_arguments, streamed_tool_call, target_arguments,
};
use common::{final_assistant_message, production_tags, settle, tool_answers};

const PARENT: &str = "E2E-PARENT-A1";
const SUBTASK: &str = "E2E-TASK-A1";

/// Plan scenario 1: parent tool call → B spawned (exact lineage tags, `paused`
/// gone, `sub · ` title) → B answered by the mock → parent gets exactly one
/// `tool` answer byte-identical to B's final assistant content, demonstrably
/// fed into the parent's next provider request → parent loop resumes. B is
/// requested exactly once (nothing was requested while it was `paused`).
#[tokio::test]
async fn e2e_sync_spawn_round_trip() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        env.listener.register(
            PARENT,
            vec![
                streamed_tool_call(
                    "call_s1_spawn",
                    "rhd_sub_chat",
                    &spawn_arguments(&[format!("{SUBTASK} research X")], false),
                ),
                routing::text_response("PARENT DONE A1"),
            ],
        );
        env.listener
            .register(SUBTASK, vec![routing::text_response("SUB ANSWER A1")]);

        let a = env.create_chat("e2e parent A1").await;
        env.wait_tools_registered(a).await;
        env.queue_user(a, &format!("do it {PARENT}")).await;

        // B exists with the contract tag set and title; `paused` gone.
        let b = wait_subchat(&env, "call_s1_spawn").await;
        wait_chat_tag(&env, b, tags::PAUSED_TAG, false).await;
        let mut expected = vec![
            tags::parent_tag(a),
            tags::root_tag(a),
            tags::call_tag("call_s1_spawn"),
        ];
        expected.sort();
        assert_eq!(
            production_tags(&env.chat_tags(b).await),
            expected,
            "B carries exactly the lineage tag set (root + link included)"
        );
        assert_eq!(
            env.chat(b).await.chat.title,
            format!("sub · {SUBTASK} research X"),
            "generated title contract"
        );
        // A itself gained the root tag (converge step 1).
        assert!(env.chat_tags(a).await.contains(&tags::root_tag(a)));

        // Exactly one answer for the call, byte-identical to B's final text.
        wait_answers(&env, a, &["call_s1_spawn"]).await;
        let answers = tool_answers(&env.client, a, "call_s1_spawn").await;
        assert_eq!(answers.len(), 1, "exactly one tool answer");
        assert_eq!(answers[0].role, "tool");
        assert_eq!(answers[0].content, "SUB ANSWER A1");
        let b_final = final_assistant_message(&env.messages(b).await).expect("B final assistant");
        assert_eq!(answers[0].content, b_final.content, "byte-identical verbatim");

        // The parent's NEXT request carries the tool answer (order proof).
        wait_for_count(&env, PARENT, 2).await;
        let parent_requests = env.listener.requests_for(PARENT);
        assert_eq!(parent_requests.len(), 2, "spawn turn + resumed turn");
        let second = &parent_requests[1];
        assert!(
            matches!(
                second.messages.as_slice(),
                [
                    ChatMessage::User { .. },
                    ChatMessage::Assistant {
                        tool_calls: Some(calls),
                        ..
                    },
                    ChatMessage::Tool {
                        tool_call_id,
                        content
                    },
                ] if calls.iter().any(|c| c.id == "call_s1_spawn")
                    && tool_call_id == "call_s1_spawn"
                    && content == "SUB ANSWER A1",
            ),
            "second parent request replays user → assistant(call) → tool(answer) \
             and is fed the answer as its newest message"
        );
        // Parent loop demonstrably resumed to a final text turn.
        wait_final_assistant(&env, a, "PARENT DONE A1").await;

        // B was requested exactly once: no provider request existed for the
        // paused window between creation and activation.
        assert_eq!(env.listener.count_for(SUBTASK), 1);
        assert_eq!(env.listener.count_for(PARENT), 2);
        // The first parent request carried the three sub-chat tools.
        let tool_names: Vec<&str> = parent_requests[0]
            .tools
            .as_ref()
            .expect("tools present")
            .iter()
            .map(|t| t.function.name.as_str())
            .collect();
        for name in ["rhd_sub_chat", "rhd_sub_chat_status", "rhd_sub_chat_await"] {
            assert!(tool_names.contains(&name), "{name} in first request");
        }

        env.shutdown();
    })
    .await
    .expect("scenario 1 timed out");
}

/// Plan scenario 2: async spawn answers with the exact background notice
/// while B streams; status reads `pending` mid-stream; await holds (loop
/// parked) until B finishes, then answers verbatim; later status reads
/// `completed`.
#[tokio::test]
async fn e2e_async_notice_and_status_await_ladder() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        let marker_parent = "E2E-PARENT-A2";
        let marker_sub = "E2E-TASK-A2";
        let (held, held_response) = routing::held_stream();
        env.listener.register(
            marker_parent,
            vec![streamed_tool_call(
                "call_s2_spawn",
                "rhd_sub_chat",
                &spawn_arguments(&[format!("{marker_sub} long task")], true),
            )],
        );
        env.listener.register(marker_sub, vec![held_response]);

        let a = env.create_chat("e2e parent A2").await;
        env.wait_tools_registered(a).await;
        env.queue_user(a, &format!("start {marker_parent}")).await;

        // Async notice: byte-exact Wire Contract string.
        wait_answers(&env, a, &["call_s2_spawn"]).await;
        let b = wait_subchat(&env, "call_s2_spawn").await;
        let notices = tool_answers(&env.client, a, "call_s2_spawn").await;
        assert_eq!(notices.len(), 1);
        assert_eq!(
            notices[0].content,
            format!(
                "Chat started in background with id {b}, use tools rhd_sub_chat_status or rhd_sub_chat_await on it"
            ),
            "golden async notice"
        );

        // B's single request is in flight (its stream is still held open).
        wait_for_count(&env, marker_sub, 1).await;

        // Status mid-stream: `pending`.
        env.listener.push(
            marker_parent,
            streamed_tool_call("call_s2_status", "rhd_sub_chat_status", &target_arguments(b)),
        );
        wait_answers(&env, a, &["call_s2_status"]).await;
        let status1 = tool_answers(&env.client, a, "call_s2_status").await;
        assert_eq!(status1.len(), 1);
        assert_eq!(status1[0].content, "pending");

        // Await registers a waiter and holds: no answer while B streams.
        env.listener.push(
            marker_parent,
            streamed_tool_call("call_s2_await", "rhd_sub_chat_await", &target_arguments(b)),
        );
        wait_assistant_call(&env, a, "call_s2_await").await;
        settle().await;
        assert!(
            tool_answers(&env.client, a, "call_s2_await").await.is_empty(),
            "await must hold while the subchat has not completed"
        );

        // Release B's answer → the parked await fires verbatim.
        routing::release_stream(&held, "SUB ANSWER A2").await;
        wait_answers(&env, a, &["call_s2_await"]).await;
        let await_answers = tool_answers(&env.client, a, "call_s2_await").await;
        assert_eq!(await_answers.len(), 1, "exactly once after the hold");
        assert_eq!(await_answers[0].content, "SUB ANSWER A2");
        let b_final = final_assistant_message(&env.messages(b).await).expect("B final");
        assert_eq!(await_answers[0].content, b_final.content);

        // After completion: status reads `completed`, parent loop ends.
        env.listener.register(
            marker_parent,
            vec![
                streamed_tool_call("call_s2_status2", "rhd_sub_chat_status", &target_arguments(b)),
                routing::text_response("PARENT DONE A2"),
            ],
        );
        wait_answers(&env, a, &["call_s2_status2"]).await;
        let status2 = tool_answers(&env.client, a, "call_s2_status2").await;
        assert_eq!(status2.len(), 1);
        assert_eq!(status2[0].content, "completed");
        wait_final_assistant(&env, a, "PARENT DONE A2").await;

        assert_eq!(env.listener.count_for(marker_sub), 1, "B streamed once");
        assert_eq!(env.listener.count_for(marker_parent), 5, "ladder turns");
        // Status/await spawned nothing: only B exists off A's call.
        assert!(find_subchat(&env.client, "call_s2_status").await.is_none());
        env.shutdown();
    })
    .await
    .expect("scenario 2 timed out");
}

/// Plan scenario 3: the subchat parks on a provider error (`ai_completions:error`
/// gate keeps status `pending`, no repeated provider requests); the operator
/// removes the error tag and queues a new message (documented ai_completions
/// semantics: un-parking ALONE does not re-request — a fresh queue item does);
/// after recovery status flips to `completed` and a fresh await answers the
/// new final text verbatim.
#[tokio::test]
async fn e2e_parked_subchat_operator_recovery() {
    timeout(Duration::from_secs(120), async {
        let env = FullEnv::start().await;
        let marker_parent = "E2E-PARENT-A3";
        let marker_sub = "E2E-TASK-A3";
        env.listener.register(
            marker_parent,
            vec![streamed_tool_call(
                "call_s3_spawn",
                "rhd_sub_chat",
                &spawn_arguments(&[format!("{marker_sub} flaky task")], true),
            )],
        );
        env.listener.register(
            marker_sub,
            vec![
                provider_error(500, "e2e boom"),
                routing::text_response("RECOVERED A3"),
            ],
        );

        let a = env.create_chat("e2e parent A3").await;
        env.wait_tools_registered(a).await;
        env.queue_user(a, &format!("start {marker_parent}")).await;
        wait_answers(&env, a, &["call_s3_spawn"]).await;
        let b = wait_subchat(&env, "call_s3_spawn").await;

        // Parked: status answers `pending`, provider retried nothing.
        env.listener.push(
            marker_parent,
            streamed_tool_call("call_s3_status", "rhd_sub_chat_status", &target_arguments(b)),
        );
        wait_answers(&env, a, &["call_s3_status"]).await;
        let status1 = tool_answers(&env.client, a, "call_s3_status").await;
        assert_eq!(status1.len(), 1);
        assert_eq!(status1[0].content, "pending");
        wait_chat_tag(&env, b, "ai_completions:error", true).await;
        settle().await;
        assert_eq!(
            env.listener.count_for(marker_sub),
            1,
            "parking must not re-request the provider"
        );

        // Operator un-parks: the error tag alone must NOT re-trigger.
        env.remove_tag(b, "ai_completions:error").await;
        settle().await;
        assert_eq!(
            env.listener.count_for(marker_sub),
            1,
            "un-parking alone does not retrigger (ai_completions semantics: \
             a new queued message is required)"
        );
        // The retry carries the sub marker so it routes to B's lane.
        env.queue_user(b, &format!("try again ({marker_sub})")).await;
        wait_final_assistant(&env, b, "RECOVERED A3").await;

        // Only now script the post-recovery ladder (a status call must not
        // race the transient states of the operator flow).
        env.listener.register(
            marker_parent,
            vec![
                streamed_tool_call("call_s3_status2", "rhd_sub_chat_status", &target_arguments(b)),
                streamed_tool_call("call_s3_await", "rhd_sub_chat_await", &target_arguments(b)),
                routing::text_response("PARENT DONE A3"),
            ],
        );
        wait_answers(&env, a, &["call_s3_status2", "call_s3_await"]).await;
        let status2 = tool_answers(&env.client, a, "call_s3_status2").await;
        assert_eq!(status2.len(), 1);
        assert_eq!(status2[0].content, "completed");
        let fresh_await = tool_answers(&env.client, a, "call_s3_await").await;
        assert_eq!(fresh_await.len(), 1);
        assert_eq!(fresh_await[0].content, "RECOVERED A3");
        let b_final = final_assistant_message(&env.messages(b).await).expect("B final");
        assert_eq!(fresh_await[0].content, b_final.content, "fresh await verbatim");
        // The original `pending` answer was never duplicated or rewritten.
        let status1 = tool_answers(&env.client, a, "call_s3_status").await;
        assert_eq!(status1.len(), 1);
        assert_eq!(status1[0].content, "pending");
        wait_final_assistant(&env, a, "PARENT DONE A3").await;

        assert_eq!(env.listener.count_for(marker_sub), 2);
        assert_eq!(env.listener.count_for(marker_parent), 5);
        env.shutdown();
    })
    .await
    .expect("scenario 3 timed out");
}
