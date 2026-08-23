use super::super::*;
use super::super::chats::ChatVersionResult;
use super::helpers::cleanup;

#[test]
fn test_create_and_list_chats() {
    let path = "test_chat_create.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id1 = db.create_chat("First").unwrap();
    let id2 = db.create_chat("Second").unwrap();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let chats = db.list_chats().unwrap();
    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0].title, "Second");
    assert_eq!(chats[1].title, "First");

    cleanup(path);
}

#[test]
fn test_get_chat() {
    let path = "test_chat_get.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.id, id);
    assert_eq!(chat.title, "Test");
    assert_eq!(chat.created_at, chat.updated_at);

    assert!(db.get_chat(999).unwrap().is_none());

    cleanup(path);
}

#[test]
fn test_delete_chat() {
    let path = "test_chat_delete.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("ToDelete").unwrap();
    assert!(db.get_chat(id).unwrap().is_some());

    db.delete_chat(id).unwrap();
    assert!(db.get_chat(id).unwrap().is_none());
    assert!(db.list_chats().unwrap().is_empty());

    cleanup(path);
}

#[test]
fn test_chat_version_initial() {
    let path = "test_chat_version_initial.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.version, 1, "New chat should have version 1");

    cleanup(path);
}

#[test]
fn test_chat_version_increments_on_title_update() {
    let path = "test_chat_version_title.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Old").unwrap();

    let chat_before = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat_before.version, 1);

    let new_version = db.update_chat_title(id, "New").unwrap();
    assert_eq!(new_version, 2, "Version should increment to 2 after title update");

    let chat_after = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat_after.version, 2);

    cleanup(path);
}

#[test]
fn test_chat_version_increments_on_touch() {
    let path = "test_chat_version_touch.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    let chat_before = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat_before.version, 1);

    let new_version = db.touch_chat(id).unwrap();
    assert_eq!(new_version, 2, "Version should increment to 2 after touch");

    let chat_after = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat_after.version, 2);

    cleanup(path);
}

#[test]
fn test_get_chat_if_version_higher() {
    let path = "test_chat_version_conditional.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Test").unwrap();

    // Chat is at version 1
    // Request with if_version_higher_than=0 should return NewerVersion
    match db.get_chat_if_version_higher(id, 0).unwrap() {
        ChatVersionResult::NewerVersion(chat) => {
            assert_eq!(chat.version, 1);
        }
        ChatVersionResult::Actual => panic!("Expected NewerVersion"),
    }

    // Request with if_version_higher_than=1 should return Actual
    match db.get_chat_if_version_higher(id, 1).unwrap() {
        ChatVersionResult::Actual => {}
        ChatVersionResult::NewerVersion(_) => panic!("Expected Actual"),
    }

    // Request with if_version_higher_than=2 should return error (version is 1, lower than requested)
    assert!(db.get_chat_if_version_higher(id, 2).is_err());

    // Update title to version 2
    db.update_chat_title(id, "Updated").unwrap();

    // Now request with if_version_higher_than=1 should return NewerVersion
    match db.get_chat_if_version_higher(id, 1).unwrap() {
        ChatVersionResult::NewerVersion(chat) => {
            assert_eq!(chat.version, 2);
        }
        ChatVersionResult::Actual => panic!("Expected NewerVersion"),
    }

    // Request with if_version_higher_than=2 should return Actual
    match db.get_chat_if_version_higher(id, 2).unwrap() {
        ChatVersionResult::Actual => {}
        ChatVersionResult::NewerVersion(_) => panic!("Expected Actual"),
    }

    cleanup(path);
}

#[test]
fn test_update_chat_title() {
    let path = "test_chat_title.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let id = db.create_chat("Old").unwrap();

    db.update_chat_title(id, "New").unwrap();
    let chat = db.get_chat(id).unwrap().unwrap();
    assert_eq!(chat.title, "New");

    cleanup(path);
}
