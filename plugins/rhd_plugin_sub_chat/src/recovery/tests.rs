//! Unit tests for the pure recovery helpers: the link-tag index build and
//! the report classification table.

use rhd_chat_api::ChatSummary;

use super::{index_chats, spawn_classification, Counter};
use crate::handler::flows::SpawnFlow;
use crate::tags;

fn summary(id: i64, tag_list: &[&str]) -> ChatSummary {
    ChatSummary {
        id,
        title: format!("chat {id}"),
        created_at: "2026-09-24T00:00:00Z".parse().unwrap(),
        updated_at: "2026-09-24T00:00:00Z".parse().unwrap(),
        tags: tag_list.iter().map(|t| t.to_string()).collect(),
        version: 1,
    }
}

/// Well-formed `sub_chat:call:<id>` tags land in the map, keyed by the bare
/// tool-call id.
#[test]
fn link_tags_are_indexed_by_tool_call_id() {
    let chats = vec![
        summary(1, &["parent:9", "paused"]),
        summary(2, &["root:9", &tags::call_tag("a")]),
        summary(3, &[&tags::call_tag("b"), "userTag"]),
    ];
    let index = index_chats(chats);
    assert_eq!(index.len(), 2);
    assert_eq!(index["a"].id, 2);
    assert_eq!(index["b"].id, 3);
}

/// Near-miss and foreign tag shapes are not ours: `sub_chat:call_` (no
/// colon), empty id, `parent:`/`root:`/bare `paused` — indexed by none, and
/// of course no panic on malformed input.
#[test]
fn near_miss_and_foreign_tags_are_ignored() {
    let chats = vec![summary(
        4,
        &[
            "sub_chat:call_",
            "sub_chat:call_x",
            "sub_chat:call:",
            "sub_chat:",
            "parent:4",
            "root:4",
            "paused",
        ],
    )];
    assert!(index_chats(chats).is_empty());
    assert!(index_chats(vec![]).is_empty());
}

/// Pathological double-create: two chats carry the same link tag. The lowest
/// chat id (earliest creation) wins **regardless of arrival order**, and the
/// duplicate is dropped without panic.
#[test]
fn duplicate_link_tags_keep_the_lowest_chat_id() {
    let dup = |first: i64, second: i64| {
        let chats = vec![
            summary(first, &[&tags::call_tag("tcD")]),
            summary(second, &[&tags::call_tag("tcD")]),
        ];
        index_chats(chats)["tcD"].id
    };
    assert_eq!(dup(9, 4), 4, "lowest id wins when it arrives second");
    assert_eq!(dup(4, 9), 4, "lowest id wins when it arrives first");
}

/// One chat may legitimately carry two link tags only under corruption;
/// both must index to it without panic (and a same-chat duplicate tag is a
/// no-op).
#[test]
fn multi_link_chat_indexes_under_both_ids() {
    let chats = vec![
        summary(5, &[&tags::call_tag("x"), &tags::call_tag("y")]),
        summary(6, &[&tags::call_tag("x")]),
    ];
    let index = index_chats(chats);
    assert_eq!(index["x"].id, 5, "5 < 6 keeps the first chat");
    assert_eq!(index["y"].id, 5);
}

/// The full classification table from the plan's dispatch section: flow
/// outcome x index entry -> report counter.
#[test]
fn classification_table() {
    let paused = summary(7, &[&tags::call_tag("tc"), tags::PAUSED_TAG]);
    let active = summary(7, &[&tags::call_tag("tc")]);
    let sync = || SpawnFlow::Sync { sub_chat_id: 7 };
    let async_ = || SpawnFlow::Async { sub_chat_id: 7 };

    // Absent B → a successful converge is a full re-spawn, either mode.
    assert_eq!(
        spawn_classification(&sync(), None),
        Counter::Respawned,
        "absent B + sync"
    );
    assert_eq!(
        spawn_classification(&async_(), None),
        Counter::Respawned,
        "absent B + async"
    );
    // Paused B → queue reconciled and activated, either mode.
    assert_eq!(
        spawn_classification(&sync(), Some(&paused)),
        Counter::ReconciledPaused
    );
    assert_eq!(
        spawn_classification(&async_(), Some(&paused)),
        Counter::ReconciledPaused
    );
    // Active B → sync re-watches; async is the late notice (answered).
    assert_eq!(
        spawn_classification(&sync(), Some(&active)),
        Counter::Rewatched
    );
    assert_eq!(
        spawn_classification(&async_(), Some(&active)),
        Counter::Answered
    );
    // Non-converge outcomes.
    assert_eq!(
        spawn_classification(&SpawnFlow::AlreadyAnswered, Some(&active)),
        Counter::SkippedAnswered
    );
    assert_eq!(
        spawn_classification(&SpawnFlow::GuardUnavailable, None),
        Counter::Untouched
    );
    assert_eq!(
        spawn_classification(&SpawnFlow::Rejected, None),
        Counter::Answered,
        "validation error answers the call"
    );
    assert_eq!(
        spawn_classification(&SpawnFlow::Failed, Some(&paused)),
        Counter::Answered,
        "tamper/error answer even while B stays paused"
    );
}
