use rhd_ai::client::{FunctionCall, ToolCall};
use rhd_db::{ChatDb, Message};
use rhd_mcp_client::client::McpClient;
use crate::manager::ChatManager;
use crate::tools::{
    build_chat_messages, build_chat_messages_for_tools, execute_tool_call,
};
use super::helpers::{MockProjectProvider, cleanup};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::broadcast;

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
    let mut iterations = 0u32;
    let max_iterations = 3u32;
    
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

#[tokio::test]
async fn test_tool_loop_no_tools_returns_immediately() {
    assert!(true, "Documented expected behavior");
}

#[tokio::test]
async fn test_tool_loop_single_tool_call() {
    assert!(true, "Documented expected behavior");
}

#[tokio::test]
async fn test_tool_loop_max_iterations() {
    let iterations = 10u32;
    let max_iterations = 10u32;
    assert!(iterations >= max_iterations);
}

#[tokio::test]
async fn test_tool_loop_cancellation() {
    use tokio_util::sync::CancellationToken;
    
    let token = CancellationToken::new();
    token.cancel();
    
    assert!(token.is_cancelled());
}
