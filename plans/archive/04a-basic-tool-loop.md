# Phase 4a: Basic Tool Execution Loop

## Goal
Enable AI to use tools from attached projects in chat conversations.

## Current State Analysis
- [`execute_ai_chat_with_tools()`](packages/rhd_app/src/scenario/ai_chat.rs:199) already implements tool loop (~260 lines)
- [`ChatManager.send_message()`](packages/rhd_app/src/chat.rs:75) uses `chat_stream_cancellable()` with streaming
- Tool execution pattern: send request → receive tool_calls → execute tools → feed results back → repeat
- Max iterations guard (default 20)

## Subtasks

### 4a.1. Extend Chat DB for tool messages
**File**: [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)

**Decision**: Store tool calls as JSON in content field (no schema change)
- `role: "tool"`, `content: JSON.stringify({toolCallId, name, arguments, result})`
- Assistant messages with tool_calls: store as JSON in content

**Migration**: No schema change needed, just new role values.

### 4a.2. Implement tool execution loop in ChatManager
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**New method**: `send_message_with_tools()`

**Logic** (adapt from [`scenario/ai_chat.rs:199`](packages/rhd_app/src/scenario/ai_chat.rs:199)):
```rust
pub async fn send_message_with_tools(
    &self,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    project_manager: &ProjectManager,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<i64, ChatError>
```

1. Get attached projects' MCP clients from ProjectManager
2. Collect all tool definitions from MCP clients
3. Add user message to DB
4. Loop:
   - Send request to AI with tools via `chat_stream_cancellable()`
   - If response has `tool_calls`:
     - For each tool call:
       - Emit `ToolCallStarted` event
       - Route to appropriate MCP client (built-in tools handled internally)
       - Execute tool
       - Emit `ToolCallCompleted` event with result
       - Add tool result to history
     - Continue loop
   - If no `tool_calls`:
     - Return final response
5. Max iterations guard (default 20)

**Key difference from scenario**: Chat tool loop streams response chunks to UI in real-time.

### 4a.3. Add chat events for tool execution
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs:30)

**New ChatEvent variants**:
```rust
ToolCallStarted {
    chat_id: i64,
    tool_call_id: String,
    tool_name: String,
    arguments: String,
},
ToolCallCompleted {
    chat_id: i64,
    tool_call_id: String,
    result: String,
},
```

**WebSocket events**:
- `toolCallStarted` - UI shows pending tool call
- `toolCallCompleted` - UI updates with result

### 4a.4. Modify send_message() to use tool loop
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs:75)

**Logic**:
- Check if chat has attached projects with MCP servers
- If yes: call `send_message_with_tools()`
- If no: call existing `send_message()` flow

## Deliverables
- [ ] Tool execution loop in [`ChatManager`](packages/rhd_app/src/chat.rs:38)
- [ ] WebSocket events: `toolCallStarted`, `toolCallCompleted`
- [ ] Integration with existing send_message flow
- [ ] Unit tests for tool execution

## Dependencies
- Phase 1 (Project data model) must be complete
- Phase 2 (ProjectManager) must be complete
- Phase 3 (Chat-Project integration) must be complete

## Risk Assessment
- **Low risk**: Reuses existing pattern from [`scenario/ai_chat.rs`](packages/rhd_app/src/scenario/ai_chat.rs:199)
- **Unknown**: How to handle very large tool results (may need truncation)
- **Mitigation**: Max iterations guard prevents infinite loops, tool errors returned as results
