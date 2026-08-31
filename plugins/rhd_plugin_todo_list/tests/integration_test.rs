//! Integration tests for the todo list plugin.

use rhd_plugin_todo_list::parser::{self, TodoStatus};
use rhd_plugin_todo_list::templates::Templates;
use rhd_plugin_todo_list::todo_store::TodoStore;

/// Test that the parser correctly handles valid markdown checklists.
#[test]
fn test_parser_valid_checklist() {
    let input = r#"[ ] Pending task
[-] In progress task
[x] Completed task
[!] Discarded task"#;

    let items = parser::parse_todo_list(input).unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].status, TodoStatus::Pending);
    assert_eq!(items[0].content, "Pending task");
    assert_eq!(items[1].status, TodoStatus::InProgress);
    assert_eq!(items[2].status, TodoStatus::Completed);
    assert_eq!(items[3].status, TodoStatus::Discarded);
}

/// Test that the parser rejects invalid formats.
#[test]
fn test_parser_invalid_format() {
    let input = "This is not a valid checklist";
    let result = parser::parse_todo_list(input);
    assert!(result.is_err());
}

/// Test that the parser handles empty input.
#[test]
fn test_parser_empty_input() {
    let input = "";
    let result = parser::parse_todo_list(input);
    assert!(result.is_err());
}

/// Test that templates can be loaded.
#[test]
fn test_templates_load() {
    let templates = Templates::load();
    assert!(templates.is_ok(), "Failed to load templates: {:?}", templates.err());
}

/// Test that the contract template contains expected content.
#[test]
fn test_contract_template_content() {
    let templates = Templates::load().unwrap();
    let contract = templates.contract();
    assert!(contract.contains("rhd_set_todo_list"));
    assert!(contract.contains("TODO list"));
}

/// Test that the tool definition is valid JSON.
#[test]
fn test_tool_definition_valid_json() {
    let templates = Templates::load().unwrap();
    let json = templates.tool_definition_json();
    assert!(json.is_ok(), "Tool definition is not valid JSON: {:?}", json.err());
    
    let json = json.unwrap();
    assert_eq!(json["name"], "rhd_set_todo_list");
    assert!(json["parameters"]["properties"]["todos"].is_object());
}

/// Test that the todo store correctly stores and retrieves items.
#[tokio::test]
async fn test_todo_store_basic_operations() {
    let store = TodoStore::new();
    
    // Initially empty
    assert!(!store.has(1).await);
    assert!(store.get(1).await.is_none());
    
    // Set items
    let items = vec![
        parser::TodoItem::new("Task 1".to_string(), TodoStatus::Pending),
        parser::TodoItem::new("Task 2".to_string(), TodoStatus::Completed),
    ];
    store.set(1, items.clone()).await;
    
    // Retrieve items
    assert!(store.has(1).await);
    let retrieved = store.get(1).await.unwrap();
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0].content, "Task 1");
    assert_eq!(retrieved[1].status, TodoStatus::Completed);
    
    // Clear items
    store.clear(1).await;
    assert!(!store.has(1).await);
}

/// Test that the todo store handles multiple chats independently.
#[tokio::test]
async fn test_todo_store_multiple_chats() {
    let store = TodoStore::new();
    
    let items1 = vec![parser::TodoItem::new("Chat 1 Task".to_string(), TodoStatus::Pending)];
    let items2 = vec![parser::TodoItem::new("Chat 2 Task".to_string(), TodoStatus::InProgress)];
    
    store.set(1, items1).await;
    store.set(2, items2).await;
    
    let retrieved1 = store.get(1).await.unwrap();
    let retrieved2 = store.get(2).await.unwrap();
    
    assert_eq!(retrieved1[0].content, "Chat 1 Task");
    assert_eq!(retrieved2[0].content, "Chat 2 Task");
    
    let ids = store.chat_ids().await;
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

/// Test rendering the todo list with items template.
#[test]
fn test_render_todo_list_with_items() {
    let templates = Templates::load().unwrap();
    let items = vec![
        parser::TodoItem::new("Analyze requirements".to_string(), TodoStatus::Completed),
        parser::TodoItem::new("Design solution".to_string(), TodoStatus::InProgress),
        parser::TodoItem::new("Implement changes".to_string(), TodoStatus::Pending),
    ];
    
    let rendered = templates.render_todo_list_with_items(&items);
    
    // Check table structure
    assert!(rendered.contains("| # | Content | Status |"));
    assert!(rendered.contains("|---|---------|--------|"));
    
    // Check items
    assert!(rendered.contains("| 1 | Analyze requirements | Completed |"));
    assert!(rendered.contains("| 2 | Design solution | In Progress |"));
    assert!(rendered.contains("| 3 | Implement changes | Pending |"));
}

/// Test the empty todo list template.
#[test]
fn test_empty_todo_list_template() {
    let templates = Templates::load().unwrap();
    let empty = templates.todo_list_empty();
    
    assert!(empty.contains("rhd_set_todo_list"));
    assert!(empty.contains("not created"));
}

/// Test the error template for invalid format.
#[test]
fn test_error_template_invalid_format() {
    let templates = Templates::load().unwrap();
    let error = templates.tool_error_invalid_format();
    
    assert!(error.contains("Invalid todo list format"));
    assert!(error.contains("[ ]"));
    assert!(error.contains("[-]"));
    assert!(error.contains("[x]"));
    assert!(error.contains("[!]"));
}

/// Test parsing tool arguments.
#[test]
fn test_parse_tool_arguments() {
    let args = r#"{"todos": "[ ] Task 1\n[x] Task 2"}"#;
    let json: serde_json::Value = serde_json::from_str(args).unwrap();
    let todos = json["todos"].as_str().unwrap();
    
    let items = parser::parse_todo_list(todos).unwrap();
    assert_eq!(items.len(), 2);
}

/// Test that the contract tag is correctly defined.
#[test]
fn test_contract_tag() {
    assert_eq!(rhd_plugin_todo_list::templates::CONTRACT_TAG, "todo_list:contract");
}
