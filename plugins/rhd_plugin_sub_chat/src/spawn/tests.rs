//! Unit tests for the spawn engine: argument validation, title/notice
//! helpers, and queue-prefix reconciliation.

use super::*;

fn starting(role: &str, content: &str) -> StartingMessage {
    StartingMessage {
        role: role.to_string(),
        content: content.to_string(),
    }
}

fn queued(id: i64, role: &str, content: &str) -> Message {
    Message {
        id,
        chat_id: 1,
        role: role.to_string(),
        content: content.to_string(),
        tool_call_id: None,
        created_at: "2026-09-23T00:00:00Z".parse().unwrap(),
        reasoning_content: None,
        tags: vec![],
        is_finished: true,
        is_streaming: false,
        tool_calls: vec![],
    }
}

// ---- parse_spawn_request ----

#[test]
fn parse_valid_sync_request() {
    let args = r#"{"messages":[{"role":"system","content":"be terse"},{"role":"user","content":"do it"}],"tags":["mcp:common"]}"#;
    let request = parse_spawn_request(args).expect("valid request");
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, "system");
    assert_eq!(request.tags, vec!["mcp:common".to_string()]);
    assert!(!request.spawn_async, "absent async flag defaults to false");
}

#[test]
fn parse_async_flag() {
    let args = r#"{"messages":[{"role":"user","content":"x"}],"async":true}"#;
    assert!(parse_spawn_request(args).unwrap().spawn_async);
    let args = r#"{"messages":[{"role":"user","content":"x"}],"async":false}"#;
    assert!(!parse_spawn_request(args).unwrap().spawn_async);
}

#[test]
fn parse_rejects_empty_messages() {
    let err = parse_spawn_request(r#"{"messages":[]}"#).unwrap_err();
    assert!(matches!(err, SpawnError::Validation(_)), "got {err:?}");
    assert!(err.to_string().contains("must not be empty"));
}

#[test]
fn parse_rejects_missing_messages_field() {
    let err = parse_spawn_request(r#"{"tags":["a"]}"#).unwrap_err();
    assert!(matches!(err, SpawnError::Validation(_)), "got {err:?}");
}

#[test]
fn parse_rejects_non_system_user_roles() {
    for role in ["assistant", "tool", "SYSTEM", ""] {
        let args = format!(r#"{{"messages":[{{"role":"{role}","content":"x"}}]}}"#);
        let err = parse_spawn_request(&args).unwrap_err();
        assert!(
            matches!(err, SpawnError::Validation(_)),
            "role {role:?} must be rejected, got {err:?}"
        );
    }
}

#[test]
fn parse_rejects_non_string_content() {
    let args = r#"{"messages":[{"role":"user","content":42}]}"#;
    let err = parse_spawn_request(args).unwrap_err();
    assert!(matches!(err, SpawnError::Validation(_)), "got {err:?}");
}

#[test]
fn parse_rejects_unparseable_json_as_validation() {
    let err = parse_spawn_request("not json").unwrap_err();
    assert!(matches!(err, SpawnError::Validation(_)), "got {err:?}");
}

#[test]
fn parse_tolerates_unknown_top_level_keys() {
    let args = r#"{"messages":[{"role":"user","content":"x"}],"unknownKey":123}"#;
    assert!(parse_spawn_request(args).is_ok());
}

#[test]
fn parse_rejects_reserved_user_tags() {
    for reserved in ["paused", "parent:5", "root:9", "sub_chat:call:x"] {
        let args =
            format!(r#"{{"messages":[{{"role":"user","content":"x"}}],"tags":["{reserved}"]}}"#);
        let err = parse_spawn_request(&args).unwrap_err();
        assert!(
            matches!(err, SpawnError::Validation(_)),
            "reserved tag {reserved:?} must be rejected, got {err:?}"
        );
    }
}

// ---- generated_title ----

#[test]
fn title_prefers_first_user_message() {
    let messages = vec![
        starting(
            "system",
            "system instructions that are quite long indeed and should not win",
        ),
        starting("user", "Count to three"),
    ];
    assert_eq!(generated_title(&messages), "sub · Count to three");
}

#[test]
fn title_truncates_at_40_chars_with_ellipsis() {
    let long = "a".repeat(45);
    let messages = vec![starting("user", &long)];
    let title = generated_title(&messages);
    assert_eq!(title, format!("sub · {}…", "a".repeat(40)));

    let exact = "b".repeat(40);
    let messages = vec![starting("user", &exact)];
    assert_eq!(generated_title(&messages), format!("sub · {exact}"));

    let one_more = "c".repeat(41);
    let messages = vec![starting("user", &one_more)];
    assert_eq!(
        generated_title(&messages),
        format!("sub · {}…", "c".repeat(40))
    );
}

#[test]
fn title_counts_chars_not_bytes() {
    // 45 Cyrillic chars (>45 bytes): truncate at 40 CHARS, not mid-UTF-8.
    let source = "привет".repeat(8); // 48 chars
    let messages = vec![starting("user", &source)];
    let title = generated_title(&messages);
    let expected: String = source.chars().take(40).collect();
    assert_eq!(title, format!("sub · {expected}…"));
}

#[test]
fn title_falls_back_to_first_message_then_constant() {
    // System-only → first (system) message content.
    let messages = vec![starting("system", "instructions")];
    assert_eq!(generated_title(&messages), "sub · instructions");
    // No messages at all → fallback constant.
    assert_eq!(generated_title(&[]), "sub · subchat");
    // Empty content is no title source either.
    let messages = vec![starting("user", "")];
    assert_eq!(generated_title(&messages), "sub · subchat");
}

// ---- async_notice (golden Wire Contract) ----

#[test]
fn async_notice_exact_string() {
    assert_eq!(
            async_notice(42),
            "Chat started in background with id 42, use tools rhd_sub_chat_status or rhd_sub_chat_await on it"
        );
}

// ---- plan_missing_suffix ----

#[test]
fn missing_suffix_empty_queue_is_everything() {
    let expected = vec![starting("system", "s"), starting("user", "u")];
    assert_eq!(plan_missing_suffix(&[], &expected).unwrap(), vec![0, 1]);
}

#[test]
fn missing_suffix_partial_prefix() {
    let expected = vec![starting("system", "s"), starting("user", "u")];
    let queue = vec![queued(1, "system", "s")];
    assert_eq!(plan_missing_suffix(&queue, &expected).unwrap(), vec![1]);
}

#[test]
fn missing_suffix_complete_queue_is_empty() {
    let expected = vec![starting("system", "s"), starting("user", "u")];
    let queue = vec![queued(1, "system", "s"), queued(2, "user", "u")];
    assert_eq!(
        plan_missing_suffix(&queue, &expected).unwrap(),
        Vec::<usize>::new()
    );
}

#[test]
fn missing_suffix_rejects_content_and_role_mismatch() {
    let expected = vec![starting("system", "s"), starting("user", "u")];
    // Content tampered.
    let queue = vec![queued(1, "system", "tampered")];
    assert_eq!(plan_missing_suffix(&queue, &expected), Err(()));
    // Role tampered.
    let queue = vec![queued(1, "user", "s")];
    assert_eq!(plan_missing_suffix(&queue, &expected), Err(()));
}

#[test]
fn missing_suffix_rejects_extra_queue_items() {
    let expected = vec![starting("user", "u")];
    let queue = vec![queued(1, "user", "u"), queued(2, "user", "injected")];
    assert_eq!(plan_missing_suffix(&queue, &expected), Err(()));
}
