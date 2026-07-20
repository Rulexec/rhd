use rhd_api::project::Role;
use rhd_db::ChatDb;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::tools::{
    collect_builtin_tools, handle_rhd_set_role, rhd_set_role_tool_definition,
};
use super::helpers::{MockProjectProvider, cleanup};
use std::sync::Arc;
use tokio::sync::broadcast;

pub fn create_mock_template_loader() -> TemplateLoaderRef {
    TemplateLoaderRef::new(|name: &str| {
        match name {
            "mcp_internal/rhd_set_todo_list/tool_definition" => Some(r#"{
                "name": "rhd_set_todo_list",
                "description": "Replace the entire TODO list with an updated checklist reflecting the current state. Always provide the full list; the system will overwrite the previous one. This tool is designed for step-by-step task tracking, allowing you to confirm completion of each step before updating, update multiple statuses at once (e.g., mark one as completed and start the next), and dynamically add new todos as they're discovered.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "string",
                            "description": "Full markdown checklist in execution order, using [ ] for pending, [x] for completed, [-] for in progress, and [!] for discarded"
                        }
                    },
                    "required": ["todos"]
                }
            }"#.to_string()),
            "mcp_internal/rhd_set_role/tool_definition" => Some(r#"{
                "name": "rhd_set_role",
                "description": "Switch the current active role. Use this tool when you need to change your behavioral role based on the task requirements. The role determines your system prompt and behavior patterns.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "role_name": {
                            "type": "string",
                            "description": "The name of the role to switch to. Must be one of the available roles listed in the system prompt."
                        }
                    },
                    "required": ["role_name"]
                }
            }"#.to_string()),
            _ => None,
        }
    })
}

#[test]
fn test_rhd_set_role_tool_definition() {
    let template_loader = create_mock_template_loader();
    let tool = rhd_set_role_tool_definition(&template_loader);
    assert_eq!(tool.function.name, "rhd_set_role");
    assert!(tool.function.description.contains("role"));
    
    let params = tool.function.parameters;
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["role_name"].is_object());
    assert_eq!(params["required"], serde_json::json!(["role_name"]));
}

#[test]
fn test_collect_builtin_tools_no_roles() {
    let db = Arc::new(ChatDb::new("test_no_roles.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let template_loader = create_mock_template_loader();
    let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id, &template_loader);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "rhd_set_todo_list");
    
    cleanup("test_no_roles.db");
}

#[test]
fn test_collect_builtin_tools_with_roles() {
    let db = Arc::new(ChatDb::new("test_with_roles.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let mut provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    provider.roles.insert("project-a".to_string(), vec![
        Role {
            name: "developer".to_string(),
            system_prompt: "Dev".to_string(),
            when_to_use: "Coding".to_string(),
        },
    ]);
    
    db.attach_project(chat_id, "project-a").unwrap();
    
    let template_loader = create_mock_template_loader();
    let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id, &template_loader);
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].function.name, "rhd_set_todo_list");
    assert_eq!(tools[1].function.name, "rhd_set_role");
    
    cleanup("test_with_roles.db");
}

#[tokio::test]
async fn test_handle_rhd_set_role_success() {
    let db = Arc::new(ChatDb::new("test_set_role.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let mut provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    provider.roles.insert("project-a".to_string(), vec![
        Role {
            name: "developer".to_string(),
            system_prompt: "You are a developer.".to_string(),
            when_to_use: "Use for coding.".to_string(),
        },
    ]);
    provider.role_prompts.insert(
        ("project-a".to_string(), "developer".to_string()),
        "You are a developer.".to_string(),
    );
    
    db.attach_project(chat_id, "project-a").unwrap();
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let args = r#"{"role_name": "developer"}"#;
    let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(false));
    assert!(result.content.contains("Successfully switched"));
    
    let active_role = db.get_active_role(chat_id).unwrap().unwrap();
    assert_eq!(active_role.0, "project-a");
    assert_eq!(active_role.1, "developer");
    
    let messages = db.get_messages(chat_id).unwrap();
    assert!(!messages.iter().any(|m| m.role == "system" && m.content.contains("Your current role is now")));
    
    assert!(db.has_role_prompt_pending(chat_id).unwrap());
    
    cleanup("test_set_role.db");
}

#[tokio::test]
async fn test_handle_rhd_set_role_not_found() {
    let db = Arc::new(ChatDb::new("test_set_role_not_found.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let args = r#"{"role_name": "nonexistent"}"#;
    let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(true));
    assert!(result.content.contains("not found"));
    
    cleanup("test_set_role_not_found.db");
}

#[tokio::test]
async fn test_handle_rhd_set_role_invalid_args() {
    let db = Arc::new(ChatDb::new("test_set_role_invalid.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let args = r#"{"invalid": "args"}"#;
    let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
    
    assert_eq!(result.is_error, Some(true));
    assert!(result.content.contains("missing required parameter"));
    
    cleanup("test_set_role_invalid.db");
}
