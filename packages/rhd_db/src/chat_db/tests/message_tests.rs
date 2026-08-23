use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_add_and_get_messages() {
    let path = "test_chat_messages.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let (msg1, _) = db.add_message(chat_id, "user", "Hello", Some("gpt4"), None).unwrap();
    let (msg2, _) = db.add_message(chat_id, "assistant", "Hi there", Some("gpt4"), None).unwrap();
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
    let (msg_id, _) = db.add_message(chat_id, "user", "Original", None, None).unwrap();

    let _version = db.update_message(msg_id, "Updated").unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.content, "Updated");

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
    db.add_queue_message(chat_id, "user", "Message 1", None, None).unwrap();
    db.add_queue_message(chat_id, "user", "Message 2", None, None).unwrap();

    // Count should be 2
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 2);

    // Delete one message
    let queue_messages = db.get_queue_messages(chat_id).unwrap();
    db.delete_queue_message(queue_messages[0].id).unwrap();

    // Count should be 1
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 1);

    cleanup(path);
}

