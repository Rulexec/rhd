use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_add_and_get_messages() {
    let path = "test_chat_messages.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let msg1 = db.add_message(chat_id, "user", "Hello", Some("gpt4"), None).unwrap();
    let msg2 = db.add_message(chat_id, "assistant", "Hi there", Some("gpt4"), None).unwrap();
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
fn test_truncate_messages() {
    let path = "test_chat_truncate.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();

    let msg1 = db.add_message(chat_id, "user", "First", None, None).unwrap();
    let _msg2 = db.add_message(chat_id, "assistant", "Second", None, None).unwrap();
    let _msg3 = db.add_message(chat_id, "user", "Third", None, None).unwrap();

    db.truncate_messages(chat_id, msg1).unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "First");

    cleanup(path);
}

#[test]
fn test_update_message() {
    let path = "test_chat_update_msg.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Chat").unwrap();
    let msg_id = db.add_message(chat_id, "user", "Original", None, None).unwrap();

    db.update_message(msg_id, "Updated").unwrap();

    let msg = db.get_message(msg_id).unwrap().unwrap();
    assert_eq!(msg.content, "Updated");

    cleanup(path);
}

#[test]
fn test_update_chat_active_model() {
    let path = "test_chat_active_model.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.active_model, None);

    db.update_chat_active_model(id, "gpt4").unwrap();
    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.active_model, Some("gpt4".to_string()));

    cleanup(path);
}
