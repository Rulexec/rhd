use std::sync::Arc;

use rhd_ai::{FunctionDefinition, ToolDefinition};
use rhd_db::ChatDb;
use rhd_mcp_client::ToolResult;
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

pub const RHD_SET_ROLE_TOOL_NAME: &str = "rhd_set_role";
pub const RHD_SET_TODO_LIST_TOOL_NAME: &str = "rhd_set_todo_list";

pub fn rhd_set_todo_list_tool_definition(template_loader: &TemplateLoaderRef) -> ToolDefinition {
    let template_content = template_loader
        .get_template("mcp_internal/rhd_set_todo_list/tool_definition")
        .expect("Template 'mcp_internal/rhd_set_todo_list/tool_definition' not found");
    
    let tool_def: serde_json::Value = serde_json::from_str(&template_content)
        .expect("Failed to parse tool definition template as JSON");
    
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_TODO_LIST_TOOL_NAME.to_string(),
            description: tool_def["description"].as_str().unwrap_or("").to_string(),
            parameters: tool_def["parameters"].clone(),
        },
    }
}

pub fn rhd_set_role_tool_definition(template_loader: &TemplateLoaderRef) -> ToolDefinition {
    let template_content = template_loader
        .get_template("mcp_internal/rhd_set_role/tool_definition")
        .expect("Template 'mcp_internal/rhd_set_role/tool_definition' not found");
    
    let tool_def: serde_json::Value = serde_json::from_str(&template_content)
        .expect("Failed to parse tool definition template as JSON");
    
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_ROLE_TOOL_NAME.to_string(),
            description: tool_def["description"].as_str().unwrap_or("").to_string(),
            parameters: tool_def["parameters"].clone(),
        },
    }
}

pub fn collect_builtin_tools(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    tools.push(rhd_set_todo_list_tool_definition(template_loader));

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return tools,
    };

    let has_roles = attached_projects
        .iter()
        .any(|(project_name, _)| !project_provider.get_project_roles(project_name).is_empty());

    if has_roles {
        tools.push(rhd_set_role_tool_definition(template_loader));
    }

    tools
}

pub async fn handle_rhd_set_todo_list<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("Error: invalid arguments: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let todos = match args.get("todos").and_then(|v| v.as_str()) {
        Some(todos) => todos.to_string(),
        None => {
            return ToolResult {
                content: "Error: missing required parameter 'todos'".to_string(),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let todo_items = match parse_todo_list(&todos) {
        Ok(items) => items,
        Err(e) => {
            return ToolResult {
                content: format!("Error: failed to parse todo list: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    match manager.db().set_todo_list(chat_id, &todos) {
        Ok(()) => {
            let _ = event_sender.send(ChatEvent::TodoListUpdated {
                chat_id,
                items: todo_items,
            });
            
            ToolResult {
                content: "Todo list updated successfully.".to_string(),
                is_error: Some(false),
                raw_response: None,
            }
        }
        Err(e) => ToolResult {
            content: format!("Error: failed to save todo list: {}", e),
            is_error: Some(true),
            raw_response: None,
        },
    }
}

pub fn parse_todo_list(input: &str) -> Result<Vec<crate::TodoItem>, String> {
    let mut items = Vec::new();
    
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if let Some(content) = trimmed.strip_prefix("[ ] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Pending,
            });
        } else if let Some(content) = trimmed.strip_prefix("[-] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::InProgress,
            });
        } else if let Some(content) = trimmed.strip_prefix("[x] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Completed,
            });
        } else if let Some(content) = trimmed.strip_prefix("[!] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Discarded,
            });
        }
    }
    
    Ok(items)
}

/// Injects todo list status as a system message after tool loop iteration.
/// This message is NOT persisted to the frontend, only added to AI context.
/// Silently skips injection if required templates are missing.
pub fn inject_todo_list_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &crate::stream::TemplateLoaderRef,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // Get current todo list from database
    let todo_list_str = match manager.db().get_todo_list(chat_id)? {
        Some(list) => list,
        None => {
            // No todo list exists, inject prompt to create one
            let empty_prompt = match template_loader.get_template("environment/todo_list_empty") {
                Some(template) => template,
                None => {
                    // Template missing, skip injection silently
                    return Ok(());
                }
            };
            
            // Add as system message (will be included in AI context)
            manager.add_message_and_notify(chat_id, "system", &empty_prompt, None, None, event_sender)?;
            return Ok(());
        }
    };
    
    // Parse todo list
    let todo_items = match parse_todo_list(&todo_list_str) {
        Ok(items) => items,
        Err(_) => {
            // Failed to parse, skip injection
            return Ok(());
        }
    };
    
    // Get active role if any
    let active_role = manager.db().get_active_role(chat_id)?;
    
    // Render environment details with todo list
    let environment_content = match render_environment_details_for_injection(
        template_loader,
        &todo_items,
        active_role.as_ref(),
    ) {
        Some(content) => content,
        None => {
            // Failed to render, skip injection silently
            return Ok(());
        }
    };
    
    // Add as system message
    manager.add_message_and_notify(chat_id, "system", &environment_content, None, None, event_sender)?;
    
    Ok(())
}

/// Helper function to render environment details for injection.
/// This is a wrapper that uses TemplateLoaderRef instead of TemplateLoader.
/// Returns None if required templates are missing.
fn render_environment_details_for_injection(
    template_loader: &crate::stream::TemplateLoaderRef,
    todo_items: &[crate::TodoItem],
    active_role: Option<&(String, String)>,
) -> Option<String> {
    // Render todo items
    let todo_items_str = if todo_items.is_empty() {
        template_loader.get_template("environment/todo_list_empty")?
    } else {
        let mut items_output = String::new();
        for (idx, item) in todo_items.iter().enumerate() {
            let status_str = match item.status {
                crate::TodoStatus::Pending => "Pending",
                crate::TodoStatus::InProgress => "In Progress",
                crate::TodoStatus::Completed => "Completed",
                crate::TodoStatus::Discarded => "Discarded",
            };
            items_output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, status_str));
        }
        
        let template = template_loader.get_template("environment/todo_list_with_items")?;
        template.replace("{todoItems}", &items_output)
    };
    
    // Select template based on role
    let template_name = if active_role.is_some() {
        "environment/details_with_role"
    } else {
        "environment/details_no_role"
    };
    
    let template = template_loader.get_template(template_name)?;
    
    let mut result = template.replace("{todoItems}", &todo_items_str);
    
    // Replace role placeholder if present
    if let Some((project_name, role_name)) = active_role {
        result = result.replace("{currentRoleName}", &format!("{} ({})", role_name, project_name));
    }
    
    Some(result)
}

pub async fn handle_rhd_set_role<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("Error: invalid arguments: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let role_name = match args.get("role_name").and_then(|v| v.as_str()) {
        Some(name) => name.to_string(),
        None => {
            return ToolResult {
                content: "Error: missing required parameter 'role_name'".to_string(),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let attached_projects = match manager.db().get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(e) => {
            return ToolResult {
                content: format!("Error: failed to get attached projects: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let mut found_project = None;
    for (project_name, _) in &attached_projects {
        let roles = manager.project_provider().get_project_roles(project_name);
        if roles.iter().any(|r| r.name == role_name) {
            found_project = Some(project_name.clone());
            break;
        }
    }

    let project_name = match found_project {
        Some(p) => p,
        None => {
            return ToolResult {
                content: format!("Error: role '{}' not found in any attached project", role_name),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    match manager
        .set_active_role(chat_id, &project_name, &role_name, event_sender.clone())
        .await
    {
        Ok(()) => ToolResult {
            content: format!("Successfully switched to role '{}'", role_name),
            is_error: Some(false),
            raw_response: None,
        },
        Err(e) => ToolResult {
            content: format!("Error: failed to set role: {}", e),
            is_error: Some(true),
            raw_response: None,
        },
    }
}
