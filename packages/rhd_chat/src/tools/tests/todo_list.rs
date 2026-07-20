use rhd_db::ChatDb;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::tools::{
    parse_todo_list, handle_rhd_set_todo_list, inject_todo_list_message,
    rhd_set_todo_list_tool_definition,
};
use super::helpers::{MockProjectProvider, cleanup};
use super::role_tests::create_mock_template_loader;
use std::sync::Arc;
use tokio::sync::broadcast;

#[test]
fn test_rhd_set_todo_list_tool_definition() {
    let template_loader = create_mock_template_loader();
    let tool = rhd_set_todo_list_tool_definition(&template_loader);
    assert_eq!(tool.function.name, "rhd_set_todo_list");
    assert!(tool.function.description.contains("TODO list"));
    
    let params = tool.function.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["todos"].is_object());
    assert_eq!(params["required"], serde_json::json!(["todos"]));
}

#[test]
fn test_parse_todo_list() {
    let input = "[x] Completed task\n[-] In progress task\n[ ] Pending task\n[!] Discarded task";
    let items = parse_todo_list(input).unwrap();
    
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].content, "Completed task");
    assert_eq!(items[0].status, crate::TodoStatus::Completed);
    assert_eq!(items[1].content, "In progress task");
    assert_eq!(items[1].status, crate::TodoStatus::InProgress);
    assert_eq!(items[2].content, "Pending task");
    assert_eq!(items[2].status, crate::TodoStatus::Pending);
    assert_eq!(items[3].content, "Discarded task");
    assert_eq!(items[3].status, crate::TodoStatus::Discarded);
}

#[test]
fn test_parse_todo_list_empty_lines() {
    let input = "[x] Task 1\n\n[-] Task 2\n\n";
    let items = parse_todo_list(input).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn test_parse_todo_list_invalid_lines() {
    let input = "[x] Valid task\nInvalid line\n[-] Another valid task";
    let items = parse_todo_list(input).unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn test_handle_rhd_set_todo_list_success() {
    let db = Arc::new(ChatDb::new("test_todo_list.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let args = r#"{"todos": "[x] Task 1\n[-] Task 2\n[ ] Task 3"}"#;
    let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(false));
    assert!(result.content.contains("successfully"));
    
    let saved = db.get_todo_list(chat_id).unwrap().unwrap();
    assert!(saved.contains("Task 1"));
    
    cleanup("test_todo_list.db");
}

#[tokio::test]
async fn test_handle_rhd_set_todo_list_invalid_args() {
    let db = Arc::new(ChatDb::new("test_todo_list_invalid.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let args = r#"{"invalid": "args"}"#;
    let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(true));
    assert!(result.content.contains("missing required parameter"));
    
    cleanup("test_todo_list_invalid.db");
}

#[tokio::test]
async fn test_handle_rhd_set_todo_list_emits_event() {
    let db = Arc::new(ChatDb::new("test_todo_event.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, mut event_receiver) = broadcast::channel(100);
    
    let args = r#"{"todos": "[x] Task 1\n[-] Task 2"}"#;
    let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(false));
    
    let event = event_receiver.recv().await.unwrap();
    match event {
        ChatEvent::TodoListUpdated { chat_id: id, items } => {
            assert_eq!(id, chat_id);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].content, "Task 1");
            assert_eq!(items[0].status, crate::TodoStatus::Completed);
            assert_eq!(items[1].content, "Task 2");
            assert_eq!(items[1].status, crate::TodoStatus::InProgress);
        }
        _ => panic!("Expected TodoListUpdated event"),
    }
    
    cleanup("test_todo_event.db");
}

#[tokio::test]
async fn test_inject_todo_list_message_with_items() {
    let db = Arc::new(ChatDb::new("test_inject_todo.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    db.set_todo_list(chat_id, "[x] Task 1\n[-] Task 2\n[ ] Task 3").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    let template_loader = crate::stream::TemplateLoaderRef::new(|name| {
        match name {
            "environment/details_no_role" => Some("<environment_details>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "environment/todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            "environment/todo_list_empty" => Some("You have not created a todo list yet.".to_string()),
            _ => None,
        }
    });
    
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("TODO list"));
    assert!(system_msg.content.contains("Task 1"));
    assert!(system_msg.content.contains("Task 2"));
    assert!(system_msg.content.contains("Task 3"));
    
    cleanup("test_inject_todo.db");
}

#[tokio::test]
async fn test_inject_todo_list_message_empty() {
    let db = Arc::new(ChatDb::new("test_inject_todo_empty.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    let template_loader = crate::stream::TemplateLoaderRef::new(|name| {
        if name == "environment/todo_list_empty" {
            Some("You have not created a todo list yet.".to_string())
        } else {
            None
        }
    });
    
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("not created a todo list"));
    
    cleanup("test_inject_todo_empty.db");
}

#[tokio::test]
async fn test_inject_todo_list_with_role() {
    let db = Arc::new(ChatDb::new("test_inject_todo_role.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    db.set_todo_list(chat_id, "[x] Task 1").unwrap();
    
    db.set_active_role(chat_id, "project-a", "developer").unwrap();
    
    let mut provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    provider.roles.insert("project-a".to_string(), vec![
        rhd_api::project::Role {
            name: "developer".to_string(),
            system_prompt: "Dev".to_string(),
            when_to_use: "Coding".to_string(),
        },
    ]);
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    let template_loader = crate::stream::TemplateLoaderRef::new(|name| {
        match name {
            "environment/details_with_role" => Some("<environment_details>\n# Current role\n<name>{currentRoleName}</name>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "environment/todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            _ => None,
        }
    });
    
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("developer (project-a)"));
    
    cleanup("test_inject_todo_role.db");
}

#[tokio::test]
async fn test_inject_todo_list_missing_template() {
    let db = Arc::new(ChatDb::new("test_inject_todo_missing.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    db.set_todo_list(chat_id, "[x] Task 1").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    let template_loader = crate::stream::TemplateLoaderRef::new(|_name| None);
    
    let (event_sender, _event_receiver) = broadcast::channel(100);
    let result = inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender);
    assert!(result.is_ok());
    
    let messages = db.get_messages(chat_id).unwrap();
    let system_messages: Vec<_> = messages.iter().filter(|m| m.role == "system").collect();
    assert_eq!(system_messages.len(), 0);
    
    cleanup("test_inject_todo_missing.db");
}
