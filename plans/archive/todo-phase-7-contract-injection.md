# Phase 7: System Message Contract Injection

## Overview

This phase injects the `rhd_set_todo_list` tool contract as a system message at the start of each chat session. This ensures the AI understands how to use the todo list tool from the beginning of the conversation.

## Files to Modify

### 1. `packages/rhd_chat/src/stream.rs`

**Add contract injection function:**

```rust
use crate::template_loader::TemplateLoader;

/// Injects the todo tool contract as a system message on first message in chat
pub fn inject_todo_tool_contract<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &TemplateLoader,
) -> Result<(), ChatError> {
    // Check if this is the first message in the chat
    let messages = manager.db().get_messages(chat_id)?;
    
    // Only inject if there are no messages yet (or only system messages)
    let has_user_messages = messages.iter().any(|m| m.role == "user");
    if has_user_messages {
        return Ok(());
    }
    
    // Check if contract has already been injected
    let has_contract = messages.iter().any(|m| {
        m.role == "system" && m.content.contains("rhd_set_todo_list Tool Contract")
    });
    if has_contract {
        return Ok(());
    }
    
    // Load contract template
    let contract_content = match template_loader.get_template("rhd_set_todo_list_contract") {
        Some(content) => content.clone(),
        None => {
            // Fallback if template is missing
            return Ok(());
        }
    };
    
    // Add as system message
    manager.db().add_message(chat_id, "system", &contract_content, None, None)?;
    
    Ok(())
}
```

**Call injection in `send_message()`:**

```rust
pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoader,  // Add this parameter
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    let pause_notify = manager.get_paused_notify(chat_id).await;

    if let Some(notify) = pause_notify {
        // ... existing pause handling ...
    }

    // Inject todo tool contract on first message
    inject_todo_tool_contract(manager, chat_id, template_loader)?;

    let model_config = models
        .get(model)
        .ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

    // ... rest of the function ...
}
```

**Call injection in `edit_and_resend()`:**

```rust
pub async fn edit_and_resend<P: ProjectProvider>(
    manager: &ChatManager<P>,
    message_id: i64,
    new_content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoader,  // Add this parameter
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let original_message = manager
        .db()
        .get_message(message_id)?
        .ok_or(ChatError::MessageNotFound)?;
    let chat_id = original_message.chat_id;
    
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    manager.db().update_message(message_id, &new_content)?;
    manager.db().truncate_messages(chat_id, message_id)?;

    // Inject todo tool contract if needed
    inject_todo_tool_contract(manager, chat_id, template_loader)?;

    // ... rest of the function ...
}
```

### 2. `packages/rhd_chat/src/tools.rs`

**Update `collect_builtin_tools()` to always include todo list tool:**

```rust
pub fn collect_builtin_tools(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // Always include todo list tool (not conditional on roles)
    tools.push(rhd_set_todo_list_tool_definition());

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return tools,
    };

    let has_roles = attached_projects
        .iter()
        .any(|(project_name, _)| !project_provider.get_project_roles(project_name).is_empty());

    if has_roles {
        tools.push(rhd_set_role_tool_definition());
    }

    tools
}
```

### 3. `packages/rhd_app/src/ws.rs`

**Pass template_loader to send_message and edit_and_resend:**

```rust
async fn handle_send_message(
    id: String,
    chat_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let chat_manager = Arc::clone(&state.chat_manager);
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let event_sender = state.chat_event_sender.clone();
    let template_loader = Arc::clone(&state.template_loader);  // Add this
    
    let state_clone = Arc::clone(state);
    tokio::spawn(async move {
        let _ = chat_manager
            .send_message(
                chat_id, 
                content, 
                &model, 
                &models, 
                event_sender, 
                &state_clone.reload_lock,
                &template_loader,  // Add this parameter
            )
            .await;
    });
    
    WsResponse::success(id, serde_json::json!({ "status": "sending" }))
}

async fn handle_edit_message(
    id: String,
    message_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let template_loader = Arc::clone(&state.template_loader);  // Add this
    
    match state
        .chat_manager
        .edit_and_resend(
            message_id,
            content,
            &model,
            &models,
            state.chat_event_sender.clone(),
            &state.reload_lock,
            &template_loader,  // Add this parameter
        )
        .await
    {
        // ... existing match arms ...
    }
}
```

### 4. `packages/rhd_chat/src/manager.rs`

**Update ChatManager methods to accept template_loader:**

```rust
impl<P: ProjectProvider> ChatManager<P> {
    pub async fn send_message(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
        reload_lock: &tokio::sync::RwLock<()>,
        template_loader: &TemplateLoader,  // Add this parameter
    ) -> Result<i64, ChatError> {
        stream::send_message(
            self,
            chat_id,
            content,
            model,
            models,
            event_sender,
            reload_lock,
            template_loader,  // Pass through
        )
        .await
    }

    pub async fn edit_and_resend(
        &self,
        message_id: i64,
        new_content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
        reload_lock: &tokio::sync::RwLock<()>,
        template_loader: &TemplateLoader,  // Add this parameter
    ) -> Result<i64, ChatError> {
        stream::edit_and_resend(
            self,
            message_id,
            new_content,
            model,
            models,
            event_sender,
            reload_lock,
            template_loader,  // Pass through
        )
        .await
    }
}
```

## Contract Content

The contract template (`templates/rhd_set_todo_list_contract.md`) should contain:

```markdown
# rhd_set_todo_list Tool Contract

## Overview

The `rhd_set_todo_list` tool manages a task tracking list for multi-step operations. It replaces the entire todo list with a new one provided as a markdown-formatted checklist.

## Tool Definition

```json
{
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
}
```

## Input Format

The `todos` parameter is a **single string** containing a markdown checklist with the following syntax:

### Checkbox States

| Syntax | Status | Description |
|--------|--------|-------------|
| `[ ]` | Pending | Task not yet started |
| `[-]` | In Progress | Task currently being worked on |
| `[x]` | Completed | Task finished |
| `[!]` | Discarded | Task no longer needed (strikethrough in UI) |

### Format Rules

1. **One item per line** - Each todo item must be on its own line
2. **Execution order** - Items should be listed in the order they will be executed
3. **Single-level list** - No nesting or subtasks allowed
4. **Full replacement** - Every call must include the complete list; partial updates are not supported

### Example Input

```
[x] Analyze the issue and design the fix
[x] Create the fix plan document
[-] Get user approval
[ ] Implement: Add database column
[ ] Implement: Modify set_active_role()
[ ] Implement: Add inject_pending_role_prompt() function
[ ] Verify: Build and test
```

## Output Format

The system renders the todo list as a formatted table in the environment details:

```
| # | Content | Status |
|---|---------|--------|
| 1 | Analyze the issue and design the fix | Completed |
| 2 | Create the fix plan document | Completed |
| 3 | Get user approval | In Progress |
| 4 | Implement: Add database column | Pending |
| 5 | Implement: Modify set_active_role() | Pending |
| 6 | Implement: Add inject_pending_role_prompt() function | Pending |
| 7 | Verify: Build and test | Pending |
```

### Status Mapping

| Input Syntax | Rendered Status |
|--------------|-----------------|
| `[ ]` | Pending |
| `[-]` | In Progress |
| `[x]` | Completed |
| `[!]` | Discarded |

## Usage Guidelines

### When to Use

- Task involves multiple steps or requires ongoing tracking
- Need to update status of several items at once
- New actionable items are discovered during execution
- Task is complex and benefits from stepwise progress tracking

### When NOT to Use

- Only a single, trivial task
- Task can be completed in one or two simple steps
- Request is purely conversational or informational

### Best Practices

1. **Update frequently** - Call the tool after completing each significant step
2. **Mark completion before starting next** - Change current item to `[x]` and next item to `[-]`
3. **Add new items dynamically** - If new tasks are discovered, append them to the list
4. **Keep all unfinished tasks** - Don't remove tasks unless explicitly completed or instructed
5. **One item per logical step** - Granularity should match meaningful progress checkpoints

### Example Workflow

**Initial call:**
```
[-] Analyze requirements
[ ] Design solution
[ ] Implement changes
[ ] Test implementation
```

**After analysis complete:**
```
[x] Analyze requirements
[-] Design solution
[ ] Implement changes
[ ] Test implementation
```

**After design complete, discovered new subtask:**
```
[x] Analyze requirements
[x] Design solution
[-] Implement changes
[ ] Add database migration
[ ] Test implementation
```

**Final state:**
```
[x] Analyze requirements
[x] Design solution
[x] Implement changes
[x] Add database migration
[x] Test implementation
```

## Response Format

On success, the tool returns:
```
Todo list updated successfully.
```

## Implementation Notes

1. The tool completely replaces the previous list - there is no merge or partial update
2. Items are automatically numbered in the rendered output (1, 2, 3, ...)
3. The system reminds the agent to update the todo list when task status changes
4. Only one todo list exists per conversation session
5. The list persists across tool calls within the same session
```

## Injection Logic

### When to Inject

1. **First message in chat** - Only when there are no user messages yet
2. **Not already injected** - Check if contract system message already exists
3. **Before user message** - Inject before processing the first user message

### Injection Flow

```
1. User sends first message
2. send_message() is called
3. inject_todo_tool_contract() checks if this is first message
4. If first message and no contract exists:
   - Load contract template
   - Add as system message
5. Continue with normal message processing
6. AI receives system message with tool contract
7. AI understands how to use rhd_set_todo_list tool
```

### Contract Persistence

- The contract is added as a system message in the database
- It persists across the entire chat session
- It's included in the conversation history sent to the AI
- It's visible in the frontend message list (can be styled differently if needed)

## Tests

### Contract Injection Tests

```rust
#[tokio::test]
async fn test_inject_todo_tool_contract_first_message() {
    let db = Arc::new(ChatDb::new("test_contract_inject.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader with contract template
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    std::fs::write(
        templates_dir.join("rhd_set_todo_list_contract.md"),
        "# rhd_set_todo_list Tool Contract\n\nThis is the contract.",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Inject contract
    inject_todo_tool_contract(&manager, chat_id, &template_loader).unwrap();
    
    // Verify system message was added
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("rhd_set_todo_list Tool Contract"));
    
    cleanup("test_contract_inject.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_not_first_message() {
    let db = Arc::new(ChatDb::new("test_contract_not_first.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Add a user message first
    db.add_message(chat_id, "user", "Hello", None, None).unwrap();
    
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
        templates_dir.join("rhd_set_todo_list_contract.md"),
        "# Contract",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Try to inject contract
    inject_todo_tool_contract(&manager, chat_id, &template_loader).unwrap();
    
    // Verify no system message was added (only the user message exists)
    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    
    cleanup("test_contract_not_first.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_already_injected() {
    let db = Arc::new(ChatDb::new("test_contract_already.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Add contract system message
    db.add_message(
        chat_id, 
        "system", 
        "# rhd_set_todo_list Tool Contract\n\nExisting contract.", 
        None, 
        None
    ).unwrap();
    
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
        templates_dir.join("rhd_set_todo_list_contract.md"),
        "# New Contract",
    ).unwrap();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Try to inject contract again
    inject_todo_tool_contract(&manager, chat_id, &template_loader).unwrap();
    
    // Verify only one system message exists (not duplicated)
    let messages = db.get_messages(chat_id).unwrap();
    let system_msgs: Vec<_> = messages.iter().filter(|m| m.role == "system").collect();
    assert_eq!(system_msgs.len(), 1);
    assert!(system_msgs[0].content.contains("Existing contract"));
    
    cleanup("test_contract_already.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_missing_template() {
    let db = Arc::new(ChatDb::new("test_contract_missing.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader without contract template
    let dir = tempfile::tempdir().unwrap();
    let templates_dir = dir.path();
    
    let template_loader = TemplateLoader::new(templates_dir).unwrap();
    
    // Try to inject contract (should not fail, just skip)
    let result = inject_todo_tool_contract(&manager, chat_id, &template_loader);
    assert!(result.is_ok());
    
    // Verify no system message was added
    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 0);
    
    cleanup("test_contract_missing.db");
}
```

### Tool Collection Tests

```rust
#[test]
fn test_collect_builtin_tools_always_includes_todo_list() {
    let db = Arc::new(ChatDb::new("test_tools_todo.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
    
    // Should always include todo list tool
    assert!(tools.iter().any(|t| t.function.name == "rhd_set_todo_list"));
    
    cleanup("test_tools_todo.db");
}

#[test]
fn test_collect_builtin_tools_with_roles_and_todo() {
    let db = Arc::new(ChatDb::new("test_tools_roles_todo.db").unwrap());
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
    
    let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
    
    // Should include both todo list and role tools
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().any(|t| t.function.name == "rhd_set_todo_list"));
    assert!(tools.iter().any(|t| t.function.name == "rhd_set_role"));
    
    cleanup("test_tools_roles_todo.db");
}
```

## Implementation Notes

1. **One-time injection**: The contract is only injected once per chat session. Subsequent messages don't trigger re-injection.

2. **Template dependency**: Uses the `rhd_set_todo_list_contract.md` template created in Phase 2.

3. **Graceful degradation**: If the template is missing, the injection is skipped silently. The tool still works, but the AI won't have explicit instructions.

4. **System message visibility**: The contract appears as a system message in the chat. Frontend can style it differently if needed (e.g., collapsible, hidden by default).

5. **Tool always available**: The `rhd_set_todo_list` tool is always included in `collect_builtin_tools()`, regardless of whether roles exist.

6. **Contract content**: The contract includes comprehensive documentation about the tool, including input format, examples, and usage guidelines.

## Dependencies

- This phase depends on Phase 2 (Templates) for the contract template
- This phase can be done in parallel with Phases 3-6
- This is the final phase in the implementation plan
