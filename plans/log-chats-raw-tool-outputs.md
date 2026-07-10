# Plan: Add Raw Tool Call Outputs to `logChatsRaw`

## Goal

Extend the existing `logChatsRaw` feature to include the **full raw MCP JSON-RPC response** from tool calls in `raw.txt`. This will allow inspection of exactly what the MCP server returns.

## Current State

- `raw.txt` currently logs:
  - Full request JSON sent to AI API
  - All raw SSE streaming chunks
  - Full non-streaming response JSON
  - Detailed error information

- Tool call results are logged to `log.txt` via `ChatLogSink::log_tool_result()`, but only the extracted content string, NOT the full MCP response.

## MCP Tool Call Response Structure

The full MCP JSON-RPC response looks like:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Tool output here"
      }
    ],
    "isError": false
  }
}
```

Currently, `call_tool()` in `McpClient` extracts only the `content` field and returns a `ToolResult` with just the text. The full response structure is lost.

## Changes Required

### 1. Modify `ToolResult` to include raw response (`packages/rhd_mcp_client/src/lib.rs`)

Add a field to store the full raw response:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    // NEW: Full raw response from MCP server
    #[serde(skip)]
    pub raw_response: Option<serde_json::Value>,
}
```

### 2. Update `call_tool()` to capture full response (`packages/rhd_mcp_client/src/client.rs`)

Modify the method to store the full `result` value:

```rust
async fn call_tool(&self, name: &str, arguments: &str) -> McpResult<ToolResult> {
    let id = self.request_id.fetch_add(1, Ordering::SeqCst);
    let parsed_arguments: serde_json::Value = serde_json::from_str(arguments)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    let params = serde_json::json!({
        "name": name,
        "arguments": parsed_arguments
    });
    let request = JsonRpcRequest::new(id, "tools/call", Some(params));
    let response = self.transport.send_request(&request).await?;

    if let Some(error) = response.error {
        return Err(McpError::Protocol(format!(
            "tools/call failed: {}",
            error.message
        )));
    }

    let result = response.result.ok_or_else(|| {
        McpError::Protocol("tools/call returned no result".to_string())
    })?;

    // NEW: Clone the full result for raw logging
    let raw_response = Some(result.clone());

    let content_value = result.get("content").ok_or_else(|| {
        McpError::Protocol("tools/call result missing 'content' field".to_string())
    })?;

    let content_string = match content_value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => content_value.to_string(),
    };

    let is_error = result.get("isError").and_then(|v| v.as_bool());

    Ok(ToolResult {
        content: content_string,
        is_error,
        raw_response, // NEW
    })
}
```

### 3. Extend `RawLogger` trait (`packages/rhd_ai/src/client.rs`)

Add a new method to log the full raw tool response:

```rust
pub trait RawLogger: Send {
    fn log_request(&mut self, request_json: &str);
    fn log_stream_chunk(&mut self, index: usize, chunk_json: &str);
    fn log_response(&mut self, response_json: &str);
    fn log_error(&mut self, status: Option<u16>, body: &str);
    // NEW:
    fn log_tool_result_raw(&mut self, tool_name: &str, call_id: &str, raw_json: &str);
}
```

### 4. Implement in `RawChatLogSink` (`packages/rhd_chat/src/chat_log.rs`)

```rust
impl rhd_ai::client::RawLogger for RawChatLogSink {
    // ... existing methods ...
    
    fn log_tool_result_raw(&mut self, tool_name: &str, call_id: &str, raw_json: &str) {
        let block = format!(
            "===== TOOL RESULT RAW: {} (id={}) =====\n{}\n",
            tool_name, call_id, raw_json
        );
        self.write_block(&block);
    }
}
```

### 5. Update `tool_loop()` to log raw tool results (`packages/rhd_chat/src/tools.rs`)

After executing each tool call, log the full raw response:

```rust
let (tool_result, mcp_id) = execute_tool_call(tool_call, mcp_clients).await;

// Log to log.txt (existing)
if let Some(ref mut l) = loggers {
    l.chat_log.log_tool_result(&tool_call.function.name, &tool_call.id, &tool_result.content);
}

// NEW: Log full raw MCP response to raw.txt
if let Some(ref mut l) = loggers {
    if let Some(ref mut raw_log) = l.raw_log {
        if let Some(ref raw_response) = tool_result.raw_response {
            let raw_json = serde_json::to_string_pretty(raw_response).unwrap_or_else(|_| raw_response.to_string());
            raw_log.log_tool_result_raw(&tool_call.function.name, &tool_call.id, &raw_json);
        }
    }
}
```

### 6. Update `execute_tool_call()` return type (`packages/rhd_chat/src/tools.rs`)

Change to return `ToolResult` instead of just the content string:

```rust
// Before:
pub async fn execute_tool_call(
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
) -> (String, String)

// After:
pub async fn execute_tool_call(
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
) -> (ToolResult, String)
```

Update the implementation to return the full `ToolResult`:

```rust
pub async fn execute_tool_call(
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
) -> (ToolResult, String) {
    let (mcp_id, bare_tool_name) = split_tool_name(&tool_call.function.name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.function.arguments).await {
                Ok(result) => return (result, client_mcp_id.clone()),
                Err(e) => return (ToolResult {
                    content: format!("Error: {}", e),
                    is_error: Some(true),
                    raw_response: None,
                }, client_mcp_id.clone()),
            }
        }
    }
    (ToolResult {
        content: format!("Error: unknown tool '{}'", tool_call.function.name),
        is_error: Some(true),
        raw_response: None,
    }, String::new())
}
```

### 7. Update tests (`packages/rhd_chat/src/tools.rs`)

Update tests to work with the new `ToolResult` return type.

## Raw Log Format Addition

The `raw.txt` will now include sections like:

```
===== TOOL RESULT RAW: mock1/echo (id=call_1) =====
{
  "content": [
    {
      "type": "text",
      "text": "Echo: Hello"
    }
  ],
  "isError": false
}
```

## Files to Modify

1. `packages/rhd_mcp_client/src/lib.rs` - Add `raw_response` field to `ToolResult`
2. `packages/rhd_mcp_client/src/client.rs` - Capture full response in `call_tool()`
3. `packages/rhd_ai/src/client.rs` - Add `log_tool_result_raw()` to `RawLogger` trait
4. `packages/rhd_chat/src/chat_log.rs` - Implement `log_tool_result_raw()` in `RawChatLogSink`
5. `packages/rhd_chat/src/tools.rs` - 
   - Change `execute_tool_call()` return type to `(ToolResult, String)`
   - Update `tool_loop()` to log raw tool results
   - Update tests

## Key Design Decisions

1. **Full MCP response logged**: The entire `result` object from the JSON-RPC response is logged, preserving the complete structure including `content` array and `isError` flag.

2. **Stored in ToolResult**: The raw response is stored in `ToolResult` with `#[serde(skip)]` so it doesn't affect serialization when sending to the AI API.

3. **Pretty-printed JSON**: The raw response is pretty-printed for readability in the log file.

4. **Separate method in RawLogger**: Keeps the interface clean and allows different formatting for raw vs. human-readable logs.
