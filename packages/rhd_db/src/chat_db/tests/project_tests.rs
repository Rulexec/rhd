use super::super::*;
use super::helpers::cleanup;

#[test]
fn test_attach_and_get_projects() {
    let path = "test_chat_projects.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-b").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].0, "project-a");
    assert_eq!(projects[0].1, false);
    assert_eq!(projects[1].0, "project-b");
    assert_eq!(projects[1].1, false);

    cleanup(path);
}

#[test]
fn test_detach_project() {
    let path = "test_chat_projects_detach.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-b").unwrap();
    db.detach_project(chat_id, "project-a").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].0, "project-b");

    cleanup(path);
}

#[test]
fn test_mark_system_prompt_added() {
    let path = "test_chat_projects_prompt.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects[0].1, false);

    db.mark_system_prompt_added(chat_id, "project-a").unwrap();
    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects[0].1, true);

    cleanup(path);
}

#[test]
fn test_attach_project_idempotent() {
    let path = "test_chat_projects_idempotent.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.attach_project(chat_id, "project-a").unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 1);

    cleanup(path);
}

#[test]
fn test_cascade_delete_projects() {
    let path = "test_chat_projects_cascade.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();

    db.attach_project(chat_id, "project-a").unwrap();
    db.delete_chat(chat_id).unwrap();

    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 0);

    cleanup(path);
}
