# Phase 1: Backend - Tool Definition & Storage

## Overview

This phase implements the core backend infrastructure for the `rhd_set_todo_list` tool, including tool definition, database storage, and handler logic.

## Files to Modify

### 1. `packages/rhd_chat/src/tools.rs`

**Additions:**

```rust
// Add constant for tool name (after RHD_SET_ROLE_TOOL_NAME)
pub const RHD_SET_TODO_LIST_TOOL_NAME: &str = "rhd_set_todo_list";

// Add tool definition function (after rhd_set_role_tool_definition)
pub fn rhd_set_todo_list_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_TODO_LIST_TOOL_NAME.to_string(),
            description: "Replace the entire TODO list with an updated checklist reflecting the current state. Always provide the full list; the system will overwrite the previous one. This tool is designed for step-by-step task tracking, allowing you to confirm completion of each step before updating, update multiple statuses at once (e.g., mark one as completed and start the next), and dynamically add new todos as they're discovered.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "string",
                        "description": "Full markdown checklist in execution order, using [ ] for pending, [x] for completed, [-] for in progress, and [!] for discarded"
                    }
                },
                "required": ["todos"]
            }),
        },
    }
}

// Add handler function (after handle_rhd_set_role)
pub async fn handle_rhd_set_todo_list(
    manager: &ChatManager<impl ProjectProvider>,
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

    // Parse and validate the todo list
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

    // Store in database
    match manager.db().set_todo_list(chat_id, &todos) {
        Ok(()) => {
            // Emit event for frontend update
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

// Add parsing function
pub fn parse_todo_list(input: &str) -> Result<Vec<crate::TodoItem>, String> {
    let mut items = Vec::new();
    
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        // Match checkbox patterns: [ ], [-], [x]
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
        // Skip lines that don't match the pattern
    }
    
    Ok(items)
}
```

**Modify `collect_builtin_tools()`:**

```rust
pub fn collect_builtin_tools(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // Always include todo list tool
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

**Modify `execute_tool_call()`:**

```rust
pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    if tool_name == RHD_SET_TODO_LIST_TOOL_NAME {
        let result = handle_rhd_set_todo_list(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    if tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = handle_rhd_set_role(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    // ... rest of the function remains the same
}
```

### 2. `packages/rhd_db/src/chat_db.rs`

**Add migration for `todo_list` column:**

```rust
// In migrate() function, add after role_prompt_pending migration:

// Check if todo_list column exists in chats table
let has_todo_list: bool = conn
    .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='todo_list'")?
    .query_row([], |row| row.get::<_, i64>(0))?
    > 0;

if !has_todo_list {
    conn.execute_batch("ALTER TABLE chats ADD COLUMN todo_list TEXT")?;
}
```

**Add methods:**

```rust
/// Sets the todo list for a chat
pub fn set_todo_list(&self, chat_id: i64, todo_list: &str) -> DbResult<()> {
    let conn = self
        .conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let now = now_iso();
    conn.execute(
        "UPDATE chats SET todo_list = ?1, updated_at = ?2 WHERE id = ?3",
        params![todo_list, now, chat_id],
    )?;
    Ok(())
}

/// Gets the todo list for a chat, returns None if not set
pub fn get_todo_list(&self, chat_id: i64) -> DbResult<Option<String>> {
    let conn = self
        .conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT todo_list FROM chats WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![chat_id], |row| {
        row.get::<_, Option<String>>(0)
    })?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok(None),
    }
}
```

**Update `ChatInfo` struct:**

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub active_model: Option<String>,
    pub active_role_project: Option<String>,
    pub active_role_name: Option<String>,
    pub todo_list: Option<String>,  // Add this field
}
```

**Update `list_chats()` and `get_chat()` queries:**

```rust
// Update SELECT queries to include todo_list
"SELECT id, title, created_at, updated_at, active_model, active_role_project, active_role_name, todo_list FROM chats ORDER BY updated_at DESC"

// Update ChatInfo mapping
Ok(ChatInfo {
    id: row.get(0)?,
    title: row.get(1)?,
    created_at: row.get(2)?,
    updated_at: row.get(3)?,
    active_model: row.get(4)?,
    active_role_project: row.get(5)?,
    active_role_name: row.get(6)?,
    todo_list: row.get(7)?,  // Add this
})
```

### 3. `packages/rhd_chat/src/lib.rs`

**Add TodoItem and TodoStatus types:**

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Discarded,
}

impl TodoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
            TodoStatus::Discarded => "discarded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

pub type TodoList = Vec<TodoItem>;
```

### 4. `packages/rhd_chat/src/event.rs`

**Add TodoListUpdated event:**

```rust
#[derive(Debug, Clone)]
pub enum ChatEvent {
    // ... existing variants ...
    TodoListUpdated {
        chat_id: i64,
        items: Vec<crate::TodoItem>,
    },
}
```

## Tests

### Unit Tests for `tools.rs`

```rust
#[test]
fn test_rhd_set_todo_list_tool_definition() {
    let tool = rhd_set_todo_list_tool_definition();
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
    
    // Verify todo list was saved
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
```

### Database Tests for `chat_db.rs`

```rust
#[test]
fn test_set_and_get_todo_list() {
    let path = "test_chat_todo_list.db";
    cleanup(path);

    let db = ChatDb::new(path).unwrap();
    let chat_id = db.create_chat("Test").unwrap();

    // Initially no todo list
    assert!(db.get_todo_list(chat_id).unwrap().is_none());

    // Set todo list
    let todo_list = "[x] Task 1\n[-] Task 2\n[ ] Task 3";
    db.set_todo_list(chat_id, todo_list).unwrap();
    
    let retrieved = db.get_todo_list(chat_id).unwrap().unwrap();
    assert_eq!(retrieved, todo_list);

    // Update todo list
    let new_todo_list = "[x] Task 1\n[x] Task 2\n[-] Task 3";
    db.set_todo_list(chat_id, new_todo_list).unwrap();
    
    let retrieved = db.get_todo_list(chat_id).unwrap().unwrap();
    assert_eq!(retrieved, new_todo_list);

    cleanup(path);
}

#[test]
fn test_migration_adds_todo_list_column() {
    let path = "test_chat_todo_migration.db";
    cleanup(path);

    // Create database with old schema
    {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE chats (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 active_model TEXT,
                 active_role_project TEXT,
                 active_role_name TEXT,
                 roles_list_injected BOOLEAN NOT NULL DEFAULT 0,
                 role_prompt_pending BOOLEAN NOT NULL DEFAULT 0
             );",
        ).unwrap();
        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
    }

    // Open with ChatDb - should trigger migration
    let db = ChatDb::new(path).unwrap();

    // Verify new column exists and works
    db.set_todo_list(1, "[x] Test task").unwrap();
    let todo = db.get_todo_list(1).unwrap().unwrap();
    assert_eq!(todo, "[x] Test task");

    cleanup(path);
}
```

## Implementation Notes

1. **Tool is always available**: Unlike `rhd_set_role`, the todo list tool is always included in `collect_builtin_tools()` regardless of whether roles exist.

2. **Database storage**: The todo list is stored as a raw markdown string in the database, not as parsed JSON. This allows for easy retrieval and re-parsing.

3. **Event emission**: When the tool is called successfully, a `TodoListUpdated` event is emitted to notify the frontend.

4. **Parsing logic**: The `parse_todo_list()` function handles all four checkbox states: `[ ]`, `[-]`, `[x]`, and `[!]`.

5. **Error handling**: Invalid arguments or parsing errors return appropriate error messages to the AI.

## Dependencies

- This phase must be completed before Phase 2 (Templates) and Phase 3 (Injection)
- Phase 4 (WebSocket) depends on the `TodoListUpdated` event defined here
