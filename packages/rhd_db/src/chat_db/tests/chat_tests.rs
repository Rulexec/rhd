use super::super::*;
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
