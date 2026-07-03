# Fix Tool Format and Add MCP Tools Logging

## Problem

1. **Tool format error**: Model API rejects tool definitions with status 400. Error shows:
   ```
   'input': {'description': '...', 'input_schema': {...}, 'name': 'rhd_set_flag'}
   ```
   Missing required `function` field. Current code sends Anthropic-style format instead of OpenAI format.

2. **No MCP tools logging**: User cannot see which tools are available to model in logs.

## Root Cause

### Issue 1: Wrong Tool Format
[`packages/rhd_ai/src/client.rs:58-63`](packages/rhd_ai/src/client.rs:58) defines:
```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

Serializes as:
```json
{"name": "...", "description": "...", "input_schema": {...}}
```

OpenAI API expects:
```json
{
  "type": "function",
  "function": {
    "name": "...",
    "description": "...",
    "parameters": {...}
  }
}
```

### Issue 2: Missing Logs
[`packages/rhd_app/src/scenario/ai_chat.rs:135-188`](packages/rhd_app/src/scenario/ai_chat.rs:135) collects tools but never logs them before sending to model.

## Solution

### Fix 1: Update ToolDefinition to OpenAI Format

**File**: [`packages/rhd_ai/src/client.rs`](packages/rhd_ai/src/client.rs)

Change `ToolDefinition` struct to match OpenAI schema:
```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,  // Always "function"
    pub function: FunctionDefinition,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Update all places that create `ToolDefinition`:
- [`packages/rhd_mcp_client/src/builtin.rs:23-42`](packages/rhd_mcp_client/src/builtin.rs:23) - `BuiltinTools::list_tool_definitions()`
- [`packages/rhd_app/src/scenario/ai_chat.rs:141-152`](packages/rhd_app/src/scenario/ai_chat.rs:141) - inline flags tool
- [`packages/rhd_app/src/scenario/ai_chat.rs:169-173`](packages/rhd_app/src/scenario/ai_chat.rs:169) - MCP tools conversion

### Fix 2: Add MCP Tools Logging

**File**: [`packages/rhd_app/src/scenario/ai_chat.rs`](packages/rhd_app/src/scenario/ai_chat.rs)

After collecting all tools (line ~188), add logging:
```rust
// Log available tools
let tool_names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
sink.log_step(step_name, "available tools", &tool_names.join(", "));
```

**File**: [`packages/rhd_app/src/log.rs`](packages/rhd_app/src/log.rs)

No changes needed - existing `log_step()` method sufficient.

## Implementation Steps

1. Update `ToolDefinition` struct in `rhd_ai/src/client.rs` to use OpenAI format
2. Add `FunctionDefinition` struct in `rhd_ai/src/client.rs`
3. Update `BuiltinTools::list_tool_definitions()` in `rhd_mcp_client/src/builtin.rs`
4. Update tool creation in `rhd_app/src/scenario/ai_chat.rs` (3 locations)
5. Add tools logging after collection in `rhd_app/src/scenario/ai_chat.rs`
6. Run `cargo build` to verify compilation
7. Run `cargo test` to verify no regressions

## Files to Modify

- [`packages/rhd_ai/src/client.rs`](packages/rhd_ai/src/client.rs) - ToolDefinition struct
- [`packages/rhd_mcp_client/src/builtin.rs`](packages/rhd_mcp_client/src/builtin.rs) - BuiltinTools
- [`packages/rhd_app/src/scenario/ai_chat.rs`](packages/rhd_app/src/scenario/ai_chat.rs) - tool creation + logging

## Success Criteria

- Model API accepts tool definitions (no 400 error)
- Logs show available tools before AI request
- All existing tests pass
- `cargo build` succeeds
