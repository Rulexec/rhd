use super::super::*;
use super::helpers::cleanup;
use crate::DbError;

#[test]
fn test_add_and_get_messages() {
    let path = "test_chat_messages.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg1, _) = db.add_message(chat_id, "user", "Hello", Some("gpt4"), None, true, false, None).unwrap();
    let (msg2, _) = db.add_message(chat_id, "assistant", "Hi there", Some("gpt4"), None, true, false, None).unwrap();
    assert_eq!(msg1, 1);
    assert_eq!(msg2, 2);

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[0].model, Some("gpt4".to_string()));
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi there");
    assert_eq!(messages[1].model, Some("gpt4".to_string()));

    cleanup(path);
}

#[test]
fn test_update_message() {
    let path = "test_chat_update_msg.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let (msg_id, _) = db.add_message(chat_id, "user", "Original", None, None, true, false, None).unwrap();

    let _version = db.update_message(msg_id, Some("Updated"), None, None, None, None).unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.content, "Updated");

    cleanup(path);
}

fn create_message_with_tool_calls(db: &ChatDb, chat_id: i64, tool_calls_json: &str) -> i64 {
    let (msg_id, _) = db
        .add_message(chat_id, "assistant", "", Some("gpt4"), None, true, false, None)
        .unwrap();
    db.update_message(msg_id, None, None, Some(tool_calls_json), None, None)
        .unwrap();
    msg_id
}

#[test]
fn test_update_tool_call_tags_add_and_remove() {
    let path = "test_chat_tool_call_tags.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    // Shape written by ai_completions plugin today: API ToolCall JSON with "type", no "tags".
    let tool_calls_json = r#"[
        {"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{}"}},
        {"id":"call_2","type":"function","function":{"name":"search","arguments":"{}"},"tags":["keep"]}
    ]"#;
    let msg_id = create_message_with_tool_calls(&db, chat_id, tool_calls_json);

    // Backward compat: tag-less stored JSON parses with empty tags.
    let msg = db.get_message(msg_id).unwrap().unwrap();
    let calls = msg.tool_calls.as_ref().unwrap();
    assert_eq!(calls[0].tags, Vec::<String>::new());
    assert_eq!(calls[1].tags, vec!["keep".to_string()]);

    let version_before = db.get_chat(chat_id).unwrap().unwrap().version;

    // Add tags to call_1 (with a duplicate to verify dedup), then change call_2 tags.
    let new_version = db
        .update_message_tool_call_tags(
            msg_id,
            "call_1",
            &["reviewed".to_string(), "reviewed".to_string()],
            &[],
        )
        .unwrap();
    let new_version = db
        .update_message_tool_call_tags(msg_id, "call_2", &["extra".to_string()], &["keep".to_string()])
        .unwrap();

    assert_eq!(new_version, version_before + 2);
    assert_eq!(db.get_chat(chat_id).unwrap().unwrap().version, new_version);

    let msg = db.get_message(msg_id).unwrap().unwrap();
    let calls = msg.tool_calls.as_ref().unwrap();
    assert_eq!(calls[0].tags, vec!["reviewed".to_string()]);
    assert_eq!(calls[1].tags, vec!["extra".to_string()]);

    cleanup(path);
}

#[test]
fn test_update_tool_call_tags_unknown_tool_call_id() {
    let path = "test_chat_tool_call_tags_404.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let tool_calls_json =
        r#"[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{}"}}]"#;
    let msg_id = create_message_with_tool_calls(&db, chat_id, tool_calls_json);

    let result = db.update_message_tool_call_tags(msg_id, "missing_id", &["x".to_string()], &[]);
    assert!(matches!(result, Err(DbError::NotFound(_))));

    // Tags unchanged after failed attempt.
    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert!(msg.tool_calls.as_ref().unwrap()[0].tags.is_empty());

    cleanup(path);
}

#[test]
fn test_update_tool_call_tags_message_without_tool_calls() {
    let path = "test_chat_tool_call_tags_none.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let (msg_id, _) = db
        .add_message(chat_id, "user", "Hello", None, None, true, false, None)
        .unwrap();

    let result = db.update_message_tool_call_tags(msg_id, "call_1", &["x".to_string()], &[]);
    assert!(matches!(result, Err(DbError::NotFound(_))));

    cleanup(path);
}

#[test]
fn test_tool_message_round_trip_with_tool_call_id() {
    let path = "test_chat_tool_call_id.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg_id, _) = db
        .add_message(chat_id, "tool", "Sunny, 22C", None, None, true, false, Some("call_abc"))
        .unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.role, "tool");
    assert_eq!(msg.tool_call_id, Some("call_abc".to_string()));

    // Also visible through the list reader.
    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages[0].tool_call_id, Some("call_abc".to_string()));

    cleanup(path);
}

#[test]
fn test_tool_call_id_none_stays_none() {
    let path = "test_chat_tool_call_id_none.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let (msg_id, _) = db
        .add_message(chat_id, "user", "Hello", None, None, true, false, None)
        .unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.tool_call_id, None);

    cleanup(path);
}

#[test]
fn test_queue_message_round_trip_with_tool_call_id() {
    let path = "test_queue_tool_call_id.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg_id, _) = db
        .add_queue_message(chat_id, "tool", "result", None, None, Some("call_xyz"))
        .unwrap();

    let msg = db.get_queue_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.tool_call_id, Some("call_xyz".to_string()));

    let messages = db.get_queue_messages(chat_id).unwrap();
    assert_eq!(messages[0].tool_call_id, Some("call_xyz".to_string()));

    cleanup(path);
}

#[test]
fn test_count_queue_messages() {
    let path = "test_count_queue_messages.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    // Initially, queue should be empty
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 0);

    // Add queue messages
    db.add_queue_message(chat_id, "user", "Message 1", None, None, None).unwrap();
    db.add_queue_message(chat_id, "user", "Message 2", None, None, None).unwrap();

    // Count should be 2
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 2);

    // Delete one message
    let queue_messages = db.get_queue_messages(chat_id).unwrap();
    db.delete_queue_message(queue_messages[0].id).unwrap();

    // Count should be 1
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 1);

    cleanup(path);
}

