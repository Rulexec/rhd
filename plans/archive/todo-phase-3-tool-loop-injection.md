# Phase 3: Backend - Todo List Injection in Tool Loop

## Overview

This phase implements the injection of todo list status into the AI conversation after each tool loop iteration. The injection provides the AI with current task tracking information to maintain context across tool calls.

## Files to Modify

### 1. `packages/rhd_chat/src/tools.rs`

**Add injection function:**

```rust
use crate::template_loader::TemplateLoader;
use std::collections::HashMap;

/// Injects todo list status as a user message after tool loop iteration
pub fn inject_todo_list_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &TemplateLoader,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // Get current todo list from database
    let todo_list_str = match manager.db().get_todo_list(chat_id)? {
        Some(list) => list,
        None => {
            // No todo list exists, inject prompt to create one
            let empty_prompt = template_loader
                .get_template("todo_list_empty")
                .map(|s| s.as_str())
                .unwrap_or("You have not created a todo list yet. Create one with `update_todo_list` if your task is complicated or involves multiple steps.");
            
            // Don't persist to DB, only send to AI
            let _ = event_sender.send(ChatEvent::TodoListInjected {
                chat_id,
                content: empty_prompt.to_string(),
            });
            
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
    let environment_content = render_environment_details(
        template_loader,
        &todo_items,
        active_role.as_ref(),
    );
    
    // Don't persist to DB, only send to AI via event
    let _ = event_sender.send(ChatEvent::TodoListInjected {
        chat_id,
        content: environment_content,
    });
    
    Ok(())
}

/// Renders environment details with todo list
fn render_environment_details(
    template_loader: &TemplateLoader,
    todo_items: &[crate::TodoItem],
    active_role: Option<&(String, String)>,
) -> String {
    let todo_items_str = if todo_items.is_empty() {
        template_loader
            .get_template("todo_list_empty")
            .map(|s| s.as_str())
            .unwrap_or("No todo list created yet.")
            .to_string()
    } else {
        let rendered_items = render_todo_items(todo_items);
        let mut replacements = HashMap::new();
        replacements.insert("todoItems".to_string(), rendered_items);
        
        template_loader
            .render_template("todo_list_with_items", &replacements)
            .unwrap_or_else(|| "Failed to render todo list.".to_string())
    };
    
    let template_name = if active_role.is_some() {
        "environment_details_with_role"
    } else {
        "environment_details_no_role"
    };
    
    let mut replacements = HashMap::new();
    replacements.insert("todoItems".to_string(), todo_items_str);
    
    if let Some((project_name, role_name)) = active_role {
        replacements.insert(
            "currentRoleName".to_string(),
            format!("{} ({})", role_name, project_name),
        );
    }
    
    template_loader
        .render_template(template_name, &replacements)
        .unwrap_or_else(|| "Failed to render environment details.".to_string())
}

/// Renders todo items as markdown table rows
fn render_todo_items(items: &[crate::TodoItem]) -> String {
    let mut output = String::new();
    
    for (idx, item) in items.iter().enumerate() {
        let status_str = match item.status {
            crate::TodoStatus::Pending => "Pending",
            crate::TodoStatus::InProgress => "In Progress",
            crate::TodoStatus::Completed => "Completed",
            crate::TodoStatus::Discarded => "Discarded",
        };
        
        output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, status_str));
    }
    
    output
}
```

**Modify `tool_loop()` to call injection:**

```rust
pub async fn tool_loop<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    model: &str,
    api_model: &str,
    client: &OpenAiClient,
    tools: &[ToolDefinition],
    mcp_clients: &[(String, String, Arc<McpClient>)],
    cancel_token: &CancellationToken,
    event_sender: &broadcast::Sender<ChatEvent>,
    iterations: &mut u32,
    current_content: &mut String,
    max_iterations: u32,
    mut loggers: Option<ChatLoggers>,
    template_loader: &TemplateLoader,  // Add this parameter
) -> Result<i64, ChatError> {
    loop {
        // ... existing loop logic ...
        
        // After tool results are processed (at the end of the loop iteration)
        // Inject todo list message for next iteration
        inject_todo_list_message(manager, chat_id, template_loader, event_sender)?;
        
        // Continue to next iteration
    }
}
```

**Update `tool_loop()` call in `stream.rs`:**

```rust
// In send_message_with_tools()
let result = tools::tool_loop(
    manager,
    chat_id,
    model,
    &model_config.model,
    &client,
    &tool_defs,
    &mcp_clients,
    &cancel_token,
    &event_sender,
    &mut iterations,
    &mut current_content,
    MAX_ITERATIONS,
    loggers,
    &state.template_loader,  // Add this parameter
)
.await;
```

### 2. `packages/rhd_chat/src/event.rs`

**Add TodoListInjected event:**

```rust
#[derive(Debug, Clone)]
pub enum ChatEvent {
    // ... existing variants ...
    TodoListInjected {
        chat_id: i64,
        content: String,
    },
}
```

### 3. `packages/rhd_app/src/ws.rs`

**Handle TodoListInjected event:**

```rust
ChatEvent::TodoListInjected { chat_id, content } => {
    // This event is not sent to frontend, only used internally
    // The content is injected as a user message to the AI
    // We need to add it to the conversation context
    
    // For now, we'll add it as a system message to the database
    // This will be picked up in the next tool loop iteration
    let _ = state.chat_manager.db().add_message(
        chat_id,
        "system",
        &content,
        None,
        None,
    );
}
```

**Alternative approach - inject directly in tool_loop:**

Instead of using events, inject the message directly in the tool loop:

```rust
pub fn inject_todo_list_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &TemplateLoader,
) -> Result<(), ChatError> {
    // Get current todo list from database
    let todo_list_str = match manager.db().get_todo_list(chat_id)? {
        Some(list) => list,
        None => {
            // No todo list exists, inject prompt to create one
            let empty_prompt = template_loader
                .get_template("todo_list_empty")
                .map(|s| s.as_str())
                .unwrap_or("You have not created a todo list yet. Create one with `update_todo_list` if your task is complicated or involves multiple steps.");
            
            // Add as system message (not persisted to frontend, only to AI context)
            manager.db().add_message(chat_id, "system", empty_prompt, None, None)?;
            
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
    let environment_content = render_environment_details(
        template_loader,
        &todo_items,
        active_role.as_ref(),
    );
    
    // Add as system message
    manager.db().add_message(chat_id, "system", &environment_content, None, None)?;
    
    Ok(())
}
```

**Call in tool_loop:**

```rust
// At the end of each tool loop iteration (after processing tool results)
inject_todo_list_message(manager, chat_id, template_loader)?;
```

### 4. `packages/rhd_app/src/daemon.rs`

**Pass template_loader to tool_loop:**

```rust
// In send_message_with_tools or wherever tool_loop is called
let result = tools::tool_loop(
    manager,
    chat_id,
    model,
    &model_config.model,
    &client,
    &tool_defs,
    &mcp_clients,
    &cancel_token,
    &event_sender,
    &mut iterations,
    &mut current_content,
    MAX_ITERATIONS,
    loggers,
    &state.template_loader,  // Pass template loader
)
.await;
```

## Injection Logic

### When to Inject

1. **After tool loop iteration completes** - All tool results have been processed
2. **Before next AI call** - The injected message will be part of the conversation context
3. **Only if todo list exists** - If no todo list, inject prompt to create one

### Injection Flow

```
1. Tool loop iteration starts
2. AI generates response with tool calls
3. Tools are executed
4. Tool results are processed
5. [NEW] Inject todo list message
6. Next iteration starts with updated context
```

### Message Type

- **System message**: Used for environment details injection
- **Not persisted to frontend**: Only added to AI conversation context
- **Overwritten each iteration**: Previous injection is replaced

### Role-Aware Injection

- If active role exists, use `environment_details_with_role.md` template
- If no active role, use `environment_details_no_role.md` template
- Include role name in format: `roleName (projectName)`

## Tests

### Injection Tests

```rust
#[tokio::test]
async fn test_inject_todo_list_message_with_items() {
    let db = Arc::new(ChatDb::new("test_inject_todo.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Set up todo list
    db.set_todo_list(chat_id, "[x] Task 1\n[-] Task 2\n[ ] Task 3").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader with test templates
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("environment_details_no_role.md"),
        "<environment_details>\n# TODO list\n{todoItems}\n</environment_details>",
    ).unwrap();
    
    std::fs::write(
        templates_dir.join("todo_list_with_items.md"),
        "| # | Content | Status |\n|---|---------|--------|\n{todoItems}",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Inject todo list
    inject_todo_list_message(&manager, chat_id, &template_loader).unwrap();
    
    // Verify system message was added
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
    
    // No todo list set
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("todo_list_empty.md"),
        "You have not created a todo list yet.",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Inject todo list
    inject_todo_list_message(&manager, chat_id, &template_loader).unwrap();
    
    // Verify system message was added with empty prompt
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("not created a todo list"));
    
    cleanup("test_inject_todo_empty.db");
}

#[tokio::test]
async fn test_inject_todo_list_with_role() {
    let db = Arc::new(ChatDb::new("test_inject_todo_role.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Set up todo list
    db.set_todo_list(chat_id, "[x] Task 1").unwrap();
    
    // Set active role
    db.set_active_role(chat_id, "project-a", "developer").unwrap();
    
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
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("environment_details_with_role.md"),
        "<environment_details>\n# Current role\n<name>{currentRoleName}</name>\n# TODO list\n{todoItems}\n</environment_details>",
    ).unwrap();
    
    std::fs::write(
        templates_dir.join("todo_list_with_items.md"),
        "| # | Content | Status |\n|---|---------|--------|\n{todoItems}",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Inject todo list
    inject_todo_list_message(&manager, chat_id, &template_loader).unwrap();
    
    // Verify system message includes role
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("developer (project-a)"));
    
    cleanup("test_inject_todo_role.db");
}
```

### Rendering Tests

```rust
#[test]
fn test_render_todo_items_table_format() {
    let items = vec![
        crate::TodoItem {
            content: "Analyze requirements".to_string(),
            status: crate::TodoStatus::Completed,
        },
        crate::TodoItem {
            content: "Design solution".to_string(),
            status: crate::TodoStatus::InProgress,
        },
        crate::TodoItem {
            content: "Implement changes".to_string(),
            status: crate::TodoStatus::Pending,
        },
    ];
    
    let rendered = render_todo_items(&items);
    
    assert!(rendered.contains("| 1 | Analyze requirements | Completed |"));
    assert!(rendered.contains("| 2 | Design solution | In Progress |"));
    assert!(rendered.contains("| 3 | Implement changes | Pending |"));
}

#[test]
fn test_render_environment_details_no_role() {
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("environment_details_no_role.md"),
        "<environment_details>\n# TODO list\n{todoItems}\n</environment_details>",
    ).unwrap();
    
    std::fs::write(
        templates_dir.join("todo_list_with_items.md"),
        "| # | Content | Status |\n|---|---------|--------|\n{todoItems}",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    let items = vec![
        crate::TodoItem {
            content: "Test task".to_string(),
            status: crate::TodoStatus::Pending,
        },
    ];
    
    let rendered = render_environment_details(&template_loader, &items, None);
    
    assert!(rendered.contains("<environment_details>"));
    assert!(rendered.contains("# TODO list"));
    assert!(rendered.contains("Test task"));
    assert!(!rendered.contains("Current role"));
}

#[test]
fn test_render_environment_details_with_role() {
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("environment_details_with_role.md"),
        "<environment_details>\n# Current role\n<name>{currentRoleName}</name>\n# TODO list\n{todoItems}\n</environment_details>",
    ).unwrap();
    
    std::fs::write(
        templates_dir.join("todo_list_with_items.md"),
        "| # | Content | Status |\n|---|---------|--------|\n{todoItems}",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    let items = vec![
        crate::TodoItem {
            content: "Test task".to_string(),
            status: crate::TodoStatus::Pending,
        },
    ];
    
    let active_role = Some(("project-a".to_string(), "developer".to_string()));
    let rendered = render_environment_details(&template_loader, &items, active_role.as_ref());
    
    assert!(rendered.contains("<environment_details>"));
    assert!(rendered.contains("# Current role"));
    assert!(rendered.contains("developer (project-a)"));
    assert!(rendered.contains("# TODO list"));
    assert!(rendered.contains("Test task"));
}
```

## Implementation Notes

1. **Injection timing**: The todo list is injected after all tool results are processed, before the next AI call. This ensures the AI has the most up-to-date task status.

2. **Message persistence**: The injected message is added to the database as a system message. This means it will be part of the conversation history and visible in the frontend.

3. **Overwriting behavior**: Each injection overwrites the previous one. The system message is added fresh each iteration, so there's no accumulation of old todo list states.

4. **Template rendering**: Uses the template loader from Phase 2 to render environment details with proper formatting.

5. **Role awareness**: The injection includes the current role if one is active, providing full context to the AI.

6. **Error handling**: If todo list parsing fails or templates are missing, the injection is skipped silently to avoid breaking the tool loop.

## Dependencies

- This phase depends on Phase 1 (Tool Definition & Storage) for database methods and types
- This phase depends on Phase 2 (Templates) for template loading and rendering
- This phase must be completed before Phase 4 (WebSocket Protocol)
