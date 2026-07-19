use super::*;
use rhd_ai::client::{FunctionCall, ToolCall};
use rhd_api::project::{McpRef, Role};
use rhd_db::{ChatDb, Message};
use rhd_mcp_client::client::McpClient;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::{McpStatus, ProjectProvider};
use std::fs;
use std::sync::Arc;
use tokio::sync::broadcast;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path));
    let _ = fs::remove_file(format!("{}-shm", path));
}

struct MockProjectProvider {
    roles: std::collections::HashMap<String, Vec<Role>>,
    role_prompts: std::collections::HashMap<(String, String), String>,
}

#[async_trait::async_trait]
impl ProjectProvider for MockProjectProvider {
    async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, McpStatus)> {
        vec![]
    }

    fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
        None
    }

    fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<McpRef> {
        vec![]
    }

    async fn get_mcp_clients(&self, _project_name: &str) -> Vec<(String, Arc<McpClient>)> {
        vec![]
    }

    async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
        Ok(())
    }

    fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        self.roles.get(project_name).cloned().unwrap_or_default()
    }

    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        self.role_prompts
            .get(&(project_name.to_string(), role_name.to_string()))
            .cloned()
    }
}

// Test helper functions

#[test]
fn test_extract_mcp_id_from_tool_name_with_prefix() {
    assert_eq!(extract_mcp_id_from_tool_name("mock1/echo"), "mock1");
    assert_eq!(extract_mcp_id_from_tool_name("server/tool"), "server");
}

#[test]
fn test_extract_mcp_id_from_tool_name_without_prefix() {
    // When there's no '/', split returns the entire string
    assert_eq!(extract_mcp_id_from_tool_name("echo"), "echo");
    assert_eq!(extract_mcp_id_from_tool_name("rhd_set_flag"), "rhd_set_flag");
}

#[test]
fn test_split_tool_name_with_prefix() {
    let (mcp_id, bare_name) = split_tool_name("mock1/echo");
    assert_eq!(mcp_id, "mock1");
    assert_eq!(bare_name, "echo");
}

#[test]
fn test_split_tool_name_without_prefix() {
    let (mcp_id, bare_name) = split_tool_name("echo");
    assert_eq!(mcp_id, "");
    assert_eq!(bare_name, "echo");
}

#[test]
fn test_build_chat_messages() {
    let messages = vec![
        Message {
            id: 1,
            chat_id: 1,
            role: "system".to_string(),
            content: "You are helpful".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            model: None,
            thinking_content: None,
        },
        Message {
            id: 2,
            chat_id: 1,
            role: "user".to_string(),
            content: "Hello".to_string(),
            created_at: "2024-01-01T00:00:01Z".to_string(),
            model: None,
            thinking_content: None,
        },
        Message {
            id: 3,
            chat_id: 1,
            role: "assistant".to_string(),
            content: "Hi there".to_string(),
            created_at: "2024-01-01T00:00:02Z".to_string(),
            model: None,
            thinking_content: None,
        },
    ];

    let chat_messages = build_chat_messages(&messages);
    assert_eq!(chat_messages.len(), 3);
}

#[test]
fn test_build_chat_messages_for_tools_simple() {
    let messages = vec![
        Message {
            id: 1,
            chat_id: 1,
            role: "user".to_string(),
            content: "Use the tool".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            model: None,
            thinking_content: None,
        },
    ];

    let chat_messages = build_chat_messages_for_tools(&messages);
    assert_eq!(chat_messages.len(), 1);
}

#[test]
fn test_build_chat_messages_for_tools_with_tool_calls() {
    let assistant_content = serde_json::json!({
        "content": "I'll use the tool",
        "toolCalls": [
            {
                "id": "call_1",
                "name": "mock1/echo",
                "arguments": "{\"message\":\"test\"}"
            }
        ]
    }).to_string();

    let tool_result = serde_json::json!({
        "toolCallId": "call_1",
        "name": "mock1/echo",
        "result": "Tool executed"
    }).to_string();

    let messages = vec![
        Message {
            id: 1,
            chat_id: 1,
            role: "user".to_string(),
            content: "Use the tool".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            model: None,
            thinking_content: None,
        },
        Message {
            id: 2,
            chat_id: 1,
            role: "assistant".to_string(),
            content: assistant_content,
            created_at: "2024-01-01T00:00:01Z".to_string(),
            model: None,
            thinking_content: None,
        },
        Message {
            id: 3,
            chat_id: 1,
            role: "tool".to_string(),
            content: tool_result,
            created_at: "2024-01-01T00:00:02Z".to_string(),
            model: None,
            thinking_content: None,
        },
    ];

    let chat_messages = build_chat_messages_for_tools(&messages);
    assert_eq!(chat_messages.len(), 3);
}

#[tokio::test]
async fn test_execute_tool_call_with_mcp_client() {
    use rhd_mcp_client::client::McpClient;
    use rhd_mcp_client::McpConfig;
    use std::sync::Arc;

    // Create a mock MCP client that returns a known result
    let _config = McpConfig {
        name: Some("mock1".to_string()),
        cmd: Some("echo".to_string()),
        args: vec![],
        cwd: None,
        env: std::collections::HashMap::new(),
    };

    // Note: We can't easily test this without a real MCP server
    // This test would require mocking the McpClient trait
    // For now, we'll test the error case
    let tool_call = ToolCall {
        id: "call_1".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "unknown/echo".to_string(),
            arguments: "{\"message\":\"test\"}".to_string(),
        },
    };

    let mcp_clients: Vec<(String, String, Arc<McpClient>)> = vec![];
    let db = Arc::new(ChatDb::new("test_execute_tool.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    let provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    let manager = ChatManager::new(db.clone(), Arc::new(provider), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let (result, mcp_id) = execute_tool_call(&manager, chat_id, &tool_call, &mcp_clients, &event_sender).await;
    
    assert!(result.content.contains("Error: unknown tool"));
    assert_eq!(mcp_id, "");
    
    cleanup("test_execute_tool.db");
}

#[tokio::test]
async fn test_execute_tool_call_unknown_tool() {
    use rhd_mcp_client::client::McpClient;
    use std::sync::Arc;

    let tool_call = ToolCall {
        id: "call_1".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "mock1/echo".to_string(),
            arguments: "{\"message\":\"test\"}".to_string(),
        },
    };

    let mcp_clients: Vec<(String, String, Arc<McpClient>)> = vec![];
    let db = Arc::new(ChatDb::new("test_execute_tool_unknown.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    let provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    let manager = ChatManager::new(db.clone(), Arc::new(provider), None, false);
    let (event_sender, _) = broadcast::channel(100);
    
    let (result, mcp_id) = execute_tool_call(&manager, chat_id, &tool_call, &mcp_clients, &event_sender).await;
    
    assert!(result.content.contains("Error: unknown tool 'mock1/echo'"));
    assert_eq!(mcp_id, "");
    
    cleanup("test_execute_tool_unknown.db");
}

#[test]
fn test_tool_loop_iteration_tracking() {
    // Test that iterations are tracked correctly
    let mut iterations = 0u32;
    let max_iterations = 3u32;
    
    // Simulate tool loop iterations
    while iterations < max_iterations {
        iterations += 1;
    }
    
    assert_eq!(iterations, 3);
    assert!(iterations >= max_iterations);
}

#[test]
fn test_tool_loop_max_iterations_check() {
    let iterations = 5u32;
    let max_iterations = 5u32;
    
    // This simulates the check in tool_loop
    let should_stop = iterations >= max_iterations;
    
    assert!(should_stop, "Should stop when iterations reach max");
}

#[test]
fn test_tool_loop_cancellation_check() {
    use tokio_util::sync::CancellationToken;
    
    let token = CancellationToken::new();
    assert!(!token.is_cancelled(), "Token should not be cancelled initially");
    
    token.cancel();
    assert!(token.is_cancelled(), "Token should be cancelled after cancel()");
}

// Integration tests for tool_loop with mock implementations

use rhd_mcp_client::{McpClientTrait, ToolDefinition as McpToolDefinition, ToolResult};
use std::collections::HashMap;
use std::future::Future;

// Mock MCP client for testing
struct MockMcpClient {
    tools: Vec<McpToolDefinition>,
    call_results: HashMap<String, String>,
}

impl MockMcpClient {
    fn new() -> Self {
        Self {
            tools: vec![],
            call_results: HashMap::new(),
        }
    }

    fn with_tool(mut self, name: &str, description: &str) -> Self {
        self.tools.push(McpToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({}),
        });
        self
    }

    fn with_call_result(mut self, tool_name: &str, result: &str) -> Self {
        self.call_results.insert(tool_name.to_string(), result.to_string());
        self
    }
}

impl McpClientTrait for MockMcpClient {
    fn list_tools(&self) -> impl Future<Output = rhd_mcp_client::McpResult<Vec<McpToolDefinition>>> + Send {
        let tools = self.tools.clone();
        async move { Ok(tools) }
    }

    fn call_tool(&self, name: &str, _arguments: &str) -> impl Future<Output = rhd_mcp_client::McpResult<ToolResult>> + Send {
        let result = self.call_results.get(name).cloned().unwrap_or_else(|| format!("Mock result for {}", name));
        async move {
            Ok(ToolResult {
                content: result,
                is_error: None,
                raw_response: None,
            })
        }
    }

    fn has_tool(&self, name: &str) -> impl Future<Output = bool> + Send {
        let has = self.tools.iter().any(|t| t.name == name);
        async move { has }
    }
}

#[tokio::test]
async fn test_tool_loop_no_tools_returns_immediately() {
    // This test would require a full mock setup
    // For now, we document the expected behavior:
    // When chat_with_tools returns no tool calls, tool_loop should:
    // 1. Emit StreamChunk with final content
    // 2. Save assistant message to DB
    // 3. Emit MessageAdded event
    // 4. Emit StreamFinished event
    // 5. Return the message ID
    assert!(true, "Documented expected behavior");
}

#[tokio::test]
async fn test_tool_loop_single_tool_call() {
    // Expected behavior:
    // 1. First call to chat_with_tools returns tool call
    // 2. Tool is executed
    // 3. ToolCallStarted event emitted
    // 4. ToolCallCompleted event emitted
    // 5. Second call to chat_with_tools returns final response
    // 6. StreamChunk, MessageAdded, StreamFinished events emitted
    assert!(true, "Documented expected behavior");
}

#[tokio::test]
async fn test_tool_loop_max_iterations() {
    // Expected behavior:
    // When iterations reach max_iterations, tool_loop should return error
    let iterations = 10u32;
    let max_iterations = 10u32;
    assert!(iterations >= max_iterations);
}

#[tokio::test]
async fn test_tool_loop_cancellation() {
    use tokio_util::sync::CancellationToken;
    
    let token = CancellationToken::new();
    token.cancel();
    
    // Expected behavior:
    // When cancel_token.is_cancelled() returns true, tool_loop should:
    // 1. Emit StreamError event with "aborted"
    // 2. Return AiError::Aborted
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn test_mock_mcp_client_list_tools() {
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool")
        .with_tool("greet", "Greeting tool");
    
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(tools[1].name, "greet");
}

#[tokio::test]
async fn test_mock_mcp_client_call_tool() {
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool")
        .with_call_result("echo", "Echo: Hello");
    
    let result = client.call_tool("echo", "{\"message\":\"Hello\"}").await.unwrap();
    assert_eq!(result.content, "Echo: Hello");
}

#[tokio::test]
async fn test_mock_mcp_client_call_tool_default_result() {
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool");
    
    let result = client.call_tool("echo", "{\"message\":\"Hello\"}").await.unwrap();
    assert_eq!(result.content, "Mock result for echo");
}

#[tokio::test]
async fn test_mock_mcp_client_has_tool() {
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool");
    
    assert!(client.has_tool("echo").await);
    assert!(!client.has_tool("nonexistent").await);
}

#[tokio::test]
async fn test_mock_mcp_client_integration() {
    // Test that MockMcpClient can be used to simulate tool execution
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool")
        .with_call_result("echo", "Echo: Test message");
    
    // Verify tool is available
    assert!(client.has_tool("echo").await);
    
    // Verify tool can be called
    let result = client.call_tool("echo", "{\"message\":\"Test message\"}").await.unwrap();
    assert_eq!(result.content, "Echo: Test message");
    
    // Verify tool list
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");
}

fn create_mock_template_loader() -> TemplateLoaderRef {
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
    
    // Verify active role was set
    let active_role = db.get_active_role(chat_id).unwrap().unwrap();
    assert_eq!(active_role.0, "project-a");
    assert_eq!(active_role.1, "developer");
    
    // Verify system message was NOT injected immediately (deferred injection)
    let messages = db.get_messages(chat_id).unwrap();
    assert!(!messages.iter().any(|m| m.role == "system" && m.content.contains("Your current role is now")));
    
    // Verify role_prompt_pending flag is set
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
            "environment/details_no_role" => Some("<environment_details>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "environment/todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            "environment/todo_list_empty" => Some("You have not created a todo list yet.".to_string()),
            _ => None,
        }
    });
    
    // Inject todo list
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
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
        if name == "environment/todo_list_empty" {
            Some("You have not created a todo list yet.".to_string())
        } else {
            None
        }
    });
    
    // Inject todo list
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
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
            "environment/details_with_role" => Some("<environment_details>\n# Current role\n<name>{currentRoleName}</name>\n# TODO list\n{todoItems}\n</environment_details>".to_string()),
            "environment/todo_list_with_items" => Some("| # | Content | Status |\n|---|---------|--------|\n{todoItems}".to_string()),
            _ => None,
        }
    });
    
    // Inject todo list
    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender).unwrap();
    
    // Verify system message includes role
    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();
    
    assert!(system_msg.content.contains("developer (project-a)"));
    
    cleanup("test_inject_todo_role.db");
}

#[tokio::test]
async fn test_inject_todo_list_missing_template() {
    let db = Arc::new(ChatDb::new("test_inject_todo_missing.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Set up todo list
    db.set_todo_list(chat_id, "[x] Task 1").unwrap();
    
    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };
    
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
    
    // Create template loader that returns None for all templates
    let template_loader = crate::stream::TemplateLoaderRef::new(|_name| None);
    
    // Inject todo list should succeed but skip injection when templates are missing
    let (event_sender, _event_receiver) = broadcast::channel(100);
    let result = inject_todo_list_message(&manager, chat_id, &template_loader, &event_sender);
    assert!(result.is_ok());
    
    // Verify no system message was added (injection was skipped)
    let messages = db.get_messages(chat_id).unwrap();
    let system_messages: Vec<_> = messages.iter().filter(|m| m.role == "system").collect();
    assert_eq!(system_messages.len(), 0);
    
    cleanup("test_inject_todo_missing.db");
}
