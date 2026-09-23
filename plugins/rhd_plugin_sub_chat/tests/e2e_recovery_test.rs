//! Phase 7 e2e, plan scenarios 7–8: the true restart-recovery story and the
//! global ack-flow guard.
//!
//! Scenario 7 deliberately touches NO production code and adds no test
//! hooks: the crash state is assembled the sanctioned way — the REAL
//! ai_completions plugin writes the parent's assistant tool_calls message
//! while the sub_chat plugin is not running yet, the mid-spawn residue
//! (paused B with a partial seed queue) is pre-created through the plain
//! chat API, and only then is `sub_chat::plugin::run_plugin` spawned fresh,
//! letting its Phase 6 startup recovery converge the state end to end.
//!
//! Crash-window coverage split: Phase 6's `tests/recovery_test.rs` walks
//! every individual window (absent B, paused B with full/partial queue,
//! late async notice, already-answered skip, tampered queue refusal,
//! idempotent re-scan) synthetically against stored history. THIS file
//! covers the one window that suite cannot: a parent history produced by
//! the real ai_completions streaming pipeline (message shape written via
//! `addMessage`-stream → `streamFinish` → `updateMessage(toolCalls)`), a
//! provider-backed subchat answer, and the full cross-plugin coordination
//! (ack flows, trigger timing, watcher drain) around the recovered call.

mod common;

use std::time::Duration;

use rhd_chat_api::{CreateChatParams, ListChatsParams};
use rhd_plugin_sub_chat::tags;
use tokio::time::timeout;

use common::full_env::FullEnv;
use common::polling::{
    wait_answers, wait_assistant_call, wait_chat_tag, wait_for_count, wait_final_assistant,
};
use common::routing::{self, spawn_arguments, streamed_tool_call};
use common::{production_tags, settle, tool_answers};

/// Plan scenario 7: real-history crash → fresh plugin start → recovery
/// converges `paused` + partial queue → completed + activated → exactly-once
/// verbatim answer into the parent's parked sync-spawn call.
#[tokio::test]
async fn e2e_crash_recovery_converges_from_real_history() {
    timeout(Duration::from_secs(120), async {
        // The sub_chat plugin is NOT started: it "crashed" before ever
        // seeing the parent's tool call.
        let mut env = FullEnv::start_without_sub_chat().await;
        let marker_parent = "E2E-PARENT-R7";
        let marker_sub = "E2E-TASK-R7";
        env.listener.register(
            marker_parent,
            vec![
                streamed_tool_call(
                    "call_r7_spawn",
                    "rhd_sub_chat",
                    &spawn_arguments(
                        &[
                            format!("{marker_sub} seed one"),
                            format!("{marker_sub} seed two"),
                        ],
                        false,
                    ),
                ),
                routing::text_response("PARENT DONE R7"),
            ],
        );
        env.listener
            .register(marker_sub, vec![routing::text_response("R RECOVERED ANSWER")]);

        let a = env.create_chat("e2e crash parent").await;
        env.queue_user(a, &format!("recover {marker_parent}")).await;

        // REAL ai_completions writes the unresolved assistant tool_calls
        // message — the "crashed between call and spawn" residue. With no
        // sub_chat plugin alive, the call stays unanswered and B is never
        // requested.
        wait_assistant_call(&env, a, "call_r7_spawn").await;
        settle().await;
        assert!(
            tool_answers(&env.client, a, "call_r7_spawn").await.is_empty(),
            "nobody answers while the plugin is down"
        );
        assert_eq!(env.listener.count_for(marker_sub), 0);

        // Mid-spawn residue: B exists `paused` with only the FIRST seed
        // queued (crash strictly between create and activation).
        let b = env
            .client
            .create_chat(CreateChatParams {
                title: "crashed mid-spawn".to_string(),
                tags: vec![
                    tags::parent_tag(a),
                    tags::root_tag(a),
                    tags::call_tag("call_r7_spawn"),
                    tags::PAUSED_TAG.to_string(),
                ],
            })
            .await
            .expect("create crashed subchat")
            .chat_id;
        env.queue_user(b, &format!("{marker_sub} seed one")).await;
        settle().await;
        assert_eq!(
            env.listener.count_for(marker_sub),
            0,
            "the paused gate must hold the crash residue"
        );

        // Fresh plugin start — its startup recovery re-drives the call.
        env.start_sub_chat_plugin().await;

        // Converged: queue completed IN ORDER + activated, subchat answered
        // by the mock, parent answered verbatim, loop resumed.
        wait_final_assistant(&env, b, "R RECOVERED ANSWER").await;
        wait_chat_tag(&env, b, tags::PAUSED_TAG, false).await;
        let mut expected = vec![
            tags::parent_tag(a),
            tags::root_tag(a),
            tags::call_tag("call_r7_spawn"),
        ];
        expected.sort();
        assert_eq!(production_tags(&env.chat_tags(b).await), expected);
        let b_messages = env.messages(b).await;
        let b_users: Vec<&str> = b_messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(
            b_users,
            vec![
                format!("{marker_sub} seed one").as_str(),
                format!("{marker_sub} seed two").as_str()
            ],
            "recovery completes the seed queue in plan order, no duplicates"
        );

        wait_answers(&env, a, &["call_r7_spawn"]).await;
        wait_final_assistant(&env, a, "PARENT DONE R7").await;
        settle().await;
        let answers = tool_answers(&env.client, a, "call_r7_spawn").await;
        assert_eq!(
            answers.len(),
            1,
            "exactly-once end to end: the recovered call is answered once"
        );
        assert_eq!(answers[0].content, "R RECOVERED ANSWER");
        assert_eq!(env.listener.count_for(marker_sub), 1);
        wait_for_count(&env, marker_parent, 2).await;
        assert_eq!(env.listener.count_for(marker_parent), 2);
        // No orphan chats: B is the ONLY sub_chat-tagged chat in existence —
        // recovery reused the paused one, never respawning a second.
        let linked: Vec<i64> = env
            .client
            .list_chats(ListChatsParams { tags: vec![] })
            .await
            .expect("listChats failed")
            .chats
            .into_iter()
            .filter(|c| c.tags.iter().any(|t| t.starts_with("sub_chat:")))
            .map(|c| c.id)
            .collect();
        assert_eq!(linked, vec![b], "recovery reuses the linked B, never respawns");
        env.shutdown();
    })
    .await
    .expect("scenario 7 timed out");
}

/// Plan scenario 8: with the sub_chat plugin registered and idle, a plain
/// user chat still completes — no `ai_completions:preRequest` stall. This
/// mirrors `rhd_plugin_choice/tests/custom_event_ack_test.rs` intent at the
/// full-pipeline level; a silent plugin would park the ack wait for 30 s and
/// then error-tag the chat, and the bounded polls below would time out.
#[tokio::test]
async fn e2e_plain_chat_completes_while_sub_chat_idle() {
    timeout(Duration::from_secs(60), async {
        let env = FullEnv::start().await;
        let marker_plain = "E2E-PLAIN-S8";
        env.listener
            .register(marker_plain, vec![routing::text_response("PLAIN ANSWER S8")]);

        let plain = env.create_chat("e2e plain chat").await;
        // The sub-chat plugin registered its tools on the plain chat...
        env.wait_tools_registered(plain).await;
        // ...and stays otherwise idle.
        env.queue_user(plain, &format!("hello {marker_plain}")).await;

        // The round trip must finish long before any 30 s ack timeout.
        wait_final_assistant(&env, plain, "PLAIN ANSWER S8").await;
        let chat = env.chat(plain).await;
        assert!(
            !chat.chat.tags.iter().any(|t| t == "ai_completions:error"),
            "plain chat must never park on the error tag: {:?}",
            chat.chat.tags
        );
        assert_eq!(env.listener.count_for(marker_plain), 1);
        // No subchat was born anywhere.
        let linked = env
            .client
            .list_chats(ListChatsParams { tags: vec![] })
            .await
            .expect("listChats failed")
            .chats
            .into_iter()
            .filter(|c| c.tags.iter().any(|t| t.starts_with("sub_chat:")))
            .count();
        assert_eq!(linked, 0);
        env.shutdown();
    })
    .await
    .expect("scenario 8 timed out");
}
