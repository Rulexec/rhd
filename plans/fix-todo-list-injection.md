# Fix: Todo List Injection in Tool Loop (Phase 3 Completion)

## Overview

This plan addresses the missing implementation of Phase 3 from the `rhd_set_todo_list` tool feature. The rendering functions and templates exist, but the actual injection logic in the tool loop is not connected.

## Problem Statement

The AI can set the todo list and the frontend displays it, but the AI does NOT receive updated todo list context during multi-step tool operations. This defeats the purpose of the todo list feature for maintaining task awareness.

## Current State

### What EXISTS:
- ✅ `render_todo_items()` in [`template_loader.rs`](packages/rhd_app/src/template_loader.rs:84)
- ✅ `render_environment_details()` in [`template_loader.rs`](packages/rhd_app/src/template_loader.rs:101)
- ✅ Template files in `templates/` directory
- ✅ `TemplateLoader` struct and loading logic
- ✅ `TemplateLoaderRef` wrapper in [`stream.rs`](packages/rhd_chat/src/stream.rs:17)

### What is MISSING:
- ❌ `inject_todo_list_message()` function in `tools.rs`
- ❌ `template_loader` parameter in `tool_loop()` function
- ❌ Injection call after tool results are processed
- ❌ Passing `template_loader` from `send_message_with_tools()` to `tool_loop()`

---

## Implementation Plan

### Step 1: Add `template_loader` Parameter to `tool_loop()`

**File:** [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs:295)

**Current signature:**
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
) -> Result<i64, ChatError>
```

**New signature:**
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
    template_loader: &crate::stream::TemplateLoaderRef,  // ADD THIS
) -> Result<i64, ChatError>
```

---

### Step 2: Create `inject_todo_list_message()` Function

**File:** [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)

Add this function after `parse_todo_list()`:

```rust
/// Injects todo list status as a system message after tool loop iteration.
/// This message is NOT persisted to the frontend, only added to AI context.
/// Returns an error if required templates are missing.
pub fn inject_todo_list_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<(), ChatError> {
    // Get current todo list from database
    let todo_list_str = match manager.db().get_todo_list(chat_id)? {
        Some(list) => list,
        None => {
            // No todo list exists, inject prompt to create one
            let empty_prompt = template_loader
                .get_template("todo_list_empty")
                .ok_or_else(|| ChatError::Internal("Missing template: todo_list_empty".to_string()))?;
            
            // Add as system message (will be included in AI context)
            manager.db().add_message(chat_id, "system", &empty_prompt, None, None)?;
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
    let environment_content = render_environment_details_for_injection(
        template_loader,
        &todo_items,
        active_role.as_ref(),
    )?;
    
    // Add as system message
    manager.db().add_message(chat_id, "system", &environment_content, None, None)?;
    
    Ok(())
}

/// Helper function to render environment details for injection.
/// This is a wrapper that uses TemplateLoaderRef instead of TemplateLoader.
/// Returns an error if required templates are missing.
fn render_environment_details_for_injection(
    template_loader: &crate::stream::TemplateLoaderRef,
    todo_items: &[crate::TodoItem],
    active_role: Option<&(String, String)>,
) -> Result<String, ChatError> {
    // Render todo items
    let todo_items_str = if todo_items.is_empty() {
        template_loader
            .get_template("todo_list_empty")
            .ok_or_else(|| ChatError::Internal("Missing template: todo_list_empty".to_string()))?
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
        
        let template = template_loader
            .get_template("todo_list_with_items")
            .ok_or_else(|| ChatError::Internal("Missing template: todo_list_with_items".to_string()))?;
        
        template.replace("{todoItems}", &items_output)
    };
    
    // Select template based on role
    let template_name = if active_role.is_some() {
        "environment_details_with_role"
    } else {
        "environment_details_no_role"
    };
    
    let template = template_loader
        .get_template(template_name)
        .ok_or_else(|| ChatError::Internal(format!("Missing template: {}", template_name)))?;
    
    let mut result = template.replace("{todoItems}", &todo_items_str);
    
    // Replace role placeholder if present
    if let Some((project_name, role_name)) = active_role {
        result = result.replace("{currentRoleName}", &format!("{} ({})", role_name, project_name));
    }
    
    Ok(result)
}
```

---

### Step 3: Call Injection in Tool Loop

**File:** [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs:557)

After tool results are processed (after the loop that saves tool messages), add the injection call:

```rust
// After line 557 (after all tool results are saved)
// Inject todo list message for next iteration
inject_todo_list_message(manager, chat_id, template_loader)?;

// Continue to next iteration (loop back)
```

**Location in context:**
```rust
// ... existing code ...
let tool_msg_id = manager.db().add_message(chat_id, "tool", &tool_result_json, None, None)?;
let tool_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded {
    chat_id,
    message: tool_message,
});
}  // End of tool execution loop

// === ADD INJECTION HERE ===
inject_todo_list_message(manager, chat_id, template_loader)?;

if let Some(content) = result.content {
    *current_content = content;
}
// Loop continues...
```

---

### Step 4: Update `send_message_with_tools()` to Pass `template_loader`

**File:** [`packages/rhd_chat/src/stream.rs`](packages/rhd_chat/src/stream.rs:207)

**Current signature:**
```rust
async fn send_message_with_tools<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    model_config: &ModelConfig,
    tool_defs: Vec<rhd_ai::ToolDefinition>,
    mcp_clients: Vec<(String, String, Arc<rhd_mcp_client::client::McpClient>)>,
    event_sender: broadcast::Sender<ChatEvent>,
    loggers: Option<chat_log::ChatLoggers>,
) -> Result<i64, ChatError>
```

**New signature:**
```rust
async fn send_message_with_tools<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    model_config: &ModelConfig,
    tool_defs: Vec<rhd_ai::ToolDefinition>,
    mcp_clients: Vec<(String, String, Arc<rhd_mcp_client::client::McpClient>)>,
    event_sender: broadcast::Sender<ChatEvent>,
    loggers: Option<chat_log::ChatLoggers>,
    template_loader: &TemplateLoaderRef,  // ADD THIS
) -> Result<i64, ChatError>
```

**Update the `tool_loop()` call:**
```rust
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
    template_loader,  // ADD THIS
)
.await;
```

---

### Step 5: Update `send_message()` to Pass `template_loader` to `send_message_with_tools()`

**File:** [`packages/rhd_chat/src/stream.rs`](packages/rhd_chat/src/stream.rs:150)

Find the call to `send_message_with_tools()` and add the `template_loader` parameter:

```rust
return send_message_with_tools(
    manager,
    chat_id,
    content,
    model,
    model_config,
    tool_defs,
    mcp_clients,
    event_sender,
    loggers,
    template_loader,  // ADD THIS
)
.await;
```

---

### Step 6: Add Tests

**File:** [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)

Add tests in the `#[cfg(test)]` module:

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
    let template_loader = crate::stream::TemplateLoaderRef::new(|name| {
        match name {
            "environment_details_no_role" => Some("<environment_details>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            "todo_list_empty" => Some("You have not created a todo list yet.".to_string()),
            _ => None,
        }
    });
    
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
    
    let template_loader = crate::stream::TemplateLoaderRef::new(|name| {
        if name == "todo_list_empty" {
            Some("You have not created a todo list yet.".to_string())
        } else {
            None
        }
    });
    
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
            "environment_details_with_role" => Some("<environment_details>\n# Current role\n<name>{currentRoleName}</name>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            _ => None,
        }
    });
    
    // Inject todo list
    inject_todo_list_message(&manager, chat_id, &template_loader).unwrap();
    
    // Verify system message includes role
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("developer (project-a)"));
    
    cleanup("test_inject_todo_role.db");
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs) | Add `template_loader` param to `tool_loop()`, add `inject_todo_list_message()`, add injection call, add tests |
| [`packages/rhd_chat/src/stream.rs`](packages/rhd_chat/src/stream.rs) | Add `template_loader` param to `send_message_with_tools()`, pass it to `tool_loop()` |

---

## Testing Checklist

- [ ] Unit test: `inject_todo_list_message()` with items
- [ ] Unit test: `inject_todo_list_message()` with empty list
- [ ] Unit test: `inject_todo_list_message()` with active role
- [ ] Integration test: Tool loop injects todo list after each iteration
- [ ] Manual test: AI receives todo list context during multi-step operations
- [ ] Manual test: Frontend displays updated todo list in real-time

---

## Expected Behavior After Fix

1. User sends a message that triggers tool usage
2. AI calls `rhd_set_todo_list` to create a task list
3. Frontend displays the todo list button with progress
4. AI executes tools in a loop
5. **After each tool iteration, the todo list is injected as a system message**
6. AI sees the current todo list state in its context
7. AI can update the todo list as tasks progress
8. Frontend reflects changes in real-time via WebSocket

---

## Notes

- The injected system message is persisted to the database (unlike the original plan which suggested not persisting)
- This is acceptable because it provides context for the AI across iterations
- The message will appear in the frontend as a system message (can be styled differently if needed)
- Future enhancement: Consider making injection optional or configurable
