use rhd_mcp_client::{McpClientTrait, ToolDefinition as McpToolDefinition, ToolResult};
use std::collections::HashMap;
use std::future::Future;

pub struct MockMcpClient {
    tools: Vec<McpToolDefinition>,
    call_results: HashMap<String, String>,
}

impl MockMcpClient {
    pub fn new() -> Self {
        Self {
            tools: vec![],
            call_results: HashMap::new(),
        }
    }

    pub fn with_tool(mut self, name: &str, description: &str) -> Self {
        self.tools.push(McpToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({}),
        });
        self
    }

    pub fn with_call_result(mut self, tool_name: &str, result: &str) -> Self {
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
    let client = MockMcpClient::new()
        .with_tool("echo", "Echo tool")
        .with_call_result("echo", "Echo: Test message");
    
    assert!(client.has_tool("echo").await);
    
    let result = client.call_tool("echo", "{\"message\":\"Test message\"}").await.unwrap();
    assert_eq!(result.content, "Echo: Test message");
    
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");
}
