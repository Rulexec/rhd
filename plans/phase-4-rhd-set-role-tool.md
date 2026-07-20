# Phase 4: Backend - rhd_set_role Tool

## Goal

Implement the `rhd_set_role` internal tool that allows the AI to switch the active role during a conversation. This tool is only available when roles are defined in attached projects.

## Current State Analysis

### Tool Infrastructure (`packages/rhd_chat/src/tools.rs`)

**Tool Collection:**
```rust
pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    // Collects tools from all attached projects' MCP clients
    // Tools are namespaced: {mcp_id}/{tool_name}
}
```

**Tool Execution:**
```rust
pub async fn execute_tool_call(
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
) -> (ToolResult, String) {
    // Parses tool name to extract mcp_id
    // Routes to appropriate MCP client
    // Returns ToolResult
}
```

**Built-in Tools:**
- `rhd_set_flag` is mentioned as a built-in tool (not namespaced)
- Built-in tools don't have MCP client association

### Tool Loop (`packages/rhd_chat/src/tools.rs`)

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
) -> Result<i64, ChatError> {
    // Loops until no tool calls or max iterations
    // Calls execute_tool_call() for each tool
    // Emits ToolCallStarted/ToolCallCompleted events
}
```

## Implementation Plan

### 4.1 Define rhd_set_role Tool Schema

**File: `packages/rhd_chat/src/tools.rs`**

Add constant for the tool definition:

```rust
pub const RHD_SET_ROLE_TOOL_NAME: &str = "rhd_set_role";

pub fn rhd_set_role_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_ROLE_TOOL_NAME.to_string(),
            description: "Switch the current active role. Use this tool when you need to change your behavioral role based on the task requirements. The role determines your system prompt and behavior patterns.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "role_name": {
                        "type": "string",
                        "description": "The name of the role to switch to. Must be one of the available roles listed in the system prompt."
                    }
                },
                "required": ["role_name"]
            }),
        },
    }
}
```

### 4.2 Add Built-in Tool Collection

**File: `packages/rhd_chat/src/tools.rs`**

Add function to collect built-in tools:

```rust
pub fn collect_builtin_tools(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    
    // Check if any attached project has roles
    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return tools,
    };
    
    let mut has_roles = false;
    for (project_name, _) in &attached_projects {
        if !project_provider.get_project_roles(project_name).is_empty() {
            has_roles = true;
            break;
        }
    }
    
    // Only add rhd_set_role if roles are available
    if has_roles {
        tools.push(rhd_set_role_tool_definition());
    }
    
    tools
}
```

### 4.3 Update Tool Collection to Include Built-in Tools

**File: `packages/rhd_chat/src/tools.rs`**

Modify `collect_tools_from_projects()` to include built-in tools:

```rust
pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    // Collect built-in tools first
    tools.extend(collect_builtin_tools(db, project_provider, chat_id));

    // Collect MCP tools from attached projects
    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return (tools, mcp_clients),
    };

    for (project_name, _) in attached_projects {
        let clients = project_provider.get_mcp_clients(&project_name).await;
        for (mcp_id, client) in clients {
            if let Ok(client_tools) = client.list_tools().await {
                for tool in client_tools {
                    let prefixed_name = format!("{}/{}", mcp_id, tool.name);
                    tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: FunctionDefinition {
                            name: prefixed_name,
                            description: tool.description,
                            parameters: tool.input_schema,
                        },
                    });
                }
                mcp_clients.push((project_name.clone(), mcp_id, client));
            }
        }
    }

    (tools, mcp_clients)
}
```

### 4.4 Implement rhd_set_role Tool Handler

**File: `packages/rhd_chat/src/tools.rs`**

Add handler function:

```rust
pub async fn handle_rhd_set_role<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    // Parse arguments
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

    // Find which project has this role
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

    // Set the active role
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
```

### 4.5 Update Tool Execution to Handle Built-in Tools

**File: `packages/rhd_chat/src/tools.rs`**

Modify `execute_tool_call()` to handle built-in tools:

```rust
pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    // Handle built-in tools
    if tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = handle_rhd_set_role(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new()); // No MCP ID for built-in tools
    }

    // Handle MCP tools
    let (mcp_id, bare_tool_name) = split_tool_name(tool_name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.function.arguments).await {
                Ok(result) => return (result, client_mcp_id.clone()),
                Err(e) => return (
                    ToolResult {
                        content: format!("Error: {}", e),
                        is_error: Some(true),
                        raw_response: None,
                    },
                    client_mcp_id.clone(),
                ),
            }
        }
    }
    
    (
        ToolResult {
            content: format!("Error: unknown tool '{}'", tool_name),
            is_error: Some(true),
            raw_response: None,
        },
        String::new(),
    )
}
```

**Note:** The signature of `execute_tool_call()` changes to accept `manager`, `chat_id`, and `event_sender`. This requires updating the call site in `tool_loop()`.

### 4.6 Update tool_loop to Pass Required Context

**File: `packages/rhd_chat/src/tools.rs`**

Update the call to `execute_tool_call()` in `tool_loop()`:

```rust
// In tool_loop(), change:
let (tool_result, _) = execute_tool_call(tool_call, mcp_clients).await;

// To:
let (tool_result, _) = execute_tool_call(manager, chat_id, tool_call, mcp_clients, event_sender).await;
```

### 4.7 Add Unit Tests

**File: `packages/rhd_chat/src/tools.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rhd_set_role_tool_definition() {
        let tool = rhd_set_role_tool_definition();
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
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        
        let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
        assert!(tools.is_empty());
        
        cleanup("test_no_roles.db");
    }

    #[test]
    fn test_collect_builtin_tools_with_roles() {
        let db = Arc::new(ChatDb::new("test_with_roles.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
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
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "rhd_set_role");
        
        cleanup("test_with_roles.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_role_success() {
        let db = Arc::new(ChatDb::new("test_set_role.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
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
        
        // Verify active role was set
        let active_role = db.get_active_role(chat_id).unwrap().unwrap();
        assert_eq!(active_role.0, "project-a");
        assert_eq!(active_role.1, "developer");
        
        // Verify system message was injected
        let messages = db.get_messages(chat_id).unwrap();
        assert!(messages.iter().any(|m| m.role == "system" && m.content.contains("Your current role is now developer")));
        
        cleanup("test_set_role.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_role_not_found() {
        let db = Arc::new(ChatDb::new("test_set_role_not_found.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
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
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
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
}
```

## Files to Modify

1. **`packages/rhd_chat/src/tools.rs`**
   - Add `RHD_SET_ROLE_TOOL_NAME` constant
   - Add `rhd_set_role_tool_definition()` function
   - Add `collect_builtin_tools()` function
   - Update `collect_tools_from_projects()` to include built-in tools
   - Add `handle_rhd_set_role()` function
   - Update `execute_tool_call()` signature and implementation
   - Update `tool_loop()` to pass required context to `execute_tool_call()`
   - Add comprehensive unit tests

## Dependencies

- Phase 1: Role struct and loading
- Phase 2: ProjectProvider trait extensions, DB methods
- Phase 3: Role injection logic, ChatManager.set_active_role()

## Success Criteria

1. ✅ `rhd_set_role` tool definition created with proper schema
2. ✅ Tool only available when roles are defined in attached projects
3. ✅ Tool handler validates role_name parameter
4. ✅ Tool handler finds the project containing the role
5. ✅ Tool handler calls `set_active_role()` on ChatManager
6. ✅ Role's system prompt is injected after successful role switch
7. ✅ Tool returns appropriate success/error messages
8. ✅ Tool is not namespaced (unlike MCP tools)
9. ✅ All unit tests pass
10. ✅ Existing tool loop tests still pass

## Design Decisions

1. **Built-in tool, not MCP tool**: `rhd_set_role` is a built-in tool that doesn't require an MCP server. It's added to the tool list conditionally based on role availability.

2. **No MCP ID prefix**: Unlike MCP tools which are namespaced as `{mcp_id}/{tool_name}`, built-in tools use just the tool name. This distinguishes them in the tool execution logic.

3. **Role lookup across projects**: The tool handler searches all attached projects to find the one containing the specified role. This allows the AI to switch roles without knowing which project defines them.

4. **Immediate injection**: When the tool is called, the role's system prompt is injected immediately as a system message. This ensures the AI's behavior changes right away.

5. **Error handling**: The tool returns detailed error messages for invalid arguments, missing roles, and other failures. This helps the AI understand what went wrong and retry if needed.

## Integration with Tool Loop

The tool loop in `tools.rs` needs to be updated to:
1. Pass `manager`, `chat_id`, and `event_sender` to `execute_tool_call()`
2. Handle the case where `execute_tool_call()` returns an empty MCP ID (for built-in tools)

The tool loop already emits `ToolCallStarted` and `ToolCallCompleted` events, which will work for `rhd_set_role` just like any other tool.

## Next Steps

After this phase:
- Phase 5 will add WebSocket protocol for role management (SetRole, GetAvailableRoles requests, RoleChanged, RolesUpdated events)
- Phase 6 will implement the frontend role selector UI
- Phase 7 will handle edge cases and integration testing
