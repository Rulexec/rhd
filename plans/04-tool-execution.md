# Phase 4: Tool Execution in Chat

## Goal
Enable AI to use tools from attached projects, implement control flow (cancel/pause/resume).

## Current State Analysis
- [`scenario/ai_chat.rs`](packages/rhd_app/src/scenario/ai_chat.rs) has tool execution loop for scenarios - can reuse pattern
- [`ChatManager.send_message()`](packages/rhd_app/src/chat.rs:75) uses `chat_stream_cancellable()` with `CancellationToken`
- [`active_streams`](packages/rhd_app/src/chat.rs:40) tracks `HashMap<i64, CancellationToken>` for abort
- [`ChatEvent`](packages/rhd_app/src/chat.rs:30) has `StreamChunk`, `StreamFinished`, `StreamError`
- Messages table stores `role` and `content` - need to extend for tool calls

## Subtasks

### 4.1. Extend ChatMessage for tool calls
**File**: [`packages/rhd_ai/src/client.rs`](packages/rhd_ai/src/client.rs)

**Current state**: Already has `Tool` variant and `tool_calls` in `Assistant`

**Changes**:
- Ensure proper serialization for DB storage
- Tool calls stored as JSON in content field or new columns

### 4.2. Extend Chat DB for tool messages
**File**: [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)

**Option A**: Store tool calls as JSON in content field
- `role: "tool"`, `content: JSON.stringify({toolCallId, name, arguments, result})`
- Simple, no schema change

**Option B**: Add new columns
- `tool_calls TEXT` - JSON array of tool calls (for assistant messages)
- `tool_call_id TEXT` - for tool result messages

**Decision**: Use Option A for simplicity. Tool calls stored as JSON in content.

**Migration**: No schema change needed, just new role values.

### 4.3. Implement tool execution loop in ChatManager
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**New method**: `send_message_with_tools()`

**Logic** (based on [`scenario/ai_chat.rs`](packages/rhd_app/src/scenario/ai_chat.rs) pattern):
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

### 4.4. Add cancel/pause/resume functionality
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Extend `active_streams`**:
```rust
enum StreamState {
    Running(CancellationToken),
    Paused {
        cancel_token: CancellationToken,
        resume_tx: oneshot::Sender<()>,
    },
}

active_streams: Mutex<HashMap<i64, StreamState>>
```

**New methods**:
```rust
pub async fn cancel_chat(&self, chat_id: i64) -> bool
```
- Cancel token immediately aborts stream

```rust
pub async fn pause_chat(&self, chat_id: i64) -> bool
```
- Wait for current tool call to complete
- Then pause before next AI request
- Store resume channel for resumption

```rust
pub async fn resume_chat(&self, chat_id: i64) -> bool
```
- Send signal via resume channel
- Tool loop continues

**Implementation detail**: Pause happens between tool calls, not mid-execution. This ensures tool results are complete before pausing.

### 4.5. Add WebSocket API for control
**File**: [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs:150)

**New WsRequest variants**:
```rust
#[serde(rename = "pauseChat", rename_all = "camelCase")]
PauseChat { id: String, chat_id: i64 },

#[serde(rename = "resumeChat", rename_all = "camelCase")]
ResumeChat { id: String, chat_id: i64 },
```

**Note**: `abortChat` already exists, reuse for cancel.

### 4.6. Add chat events for tool execution
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
ChatPaused { chat_id: i64 },
ChatResumed { chat_id: i64 },
```

**WebSocket events**:
- `toolCallStarted` - UI shows pending tool call
- `toolCallCompleted` - UI updates with result
- `chatPaused` - UI shows pause state, enables resume button
- `chatResumed` - UI hides pause state

### 4.7. Handle user message during pause
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Logic**:
- When paused, user can send message via `send_message()`
- Message appended to history after current tool results
- Resume tool loop with updated history
- Modify `send_message()` to check if chat is paused:
  - If paused, append message to DB
  - Signal resume (or let user manually resume)

**Design decision**: User message during pause automatically resumes the tool loop. This provides smooth UX.

## Deliverables
- [ ] Tool execution loop in [`ChatManager`](packages/rhd_app/src/chat.rs:38)
- [ ] Cancel/pause/resume functionality
- [ ] WebSocket API: `pauseChat`, `resumeChat` (reuse `abortChat` for cancel)
- [ ] WebSocket events: `toolCallStarted`, `toolCallCompleted`, `chatPaused`, `chatResumed`
- [ ] User message during pause handling
- [ ] Unit tests for tool execution
- [ ] Unit tests for pause/resume

## Dependencies
- Phase 1 (Project data model) must be complete
- Phase 2 (ProjectManager) must be complete
- Phase 3 (Chat-Project integration) must be complete

## Risk Assessment
- **High risk**: Tool execution loop complexity - must handle errors, timeouts, crashes
- **Unknown**: How to handle very large tool results (may need truncation)
- **Unknown**: Pause/resume state machine complexity
- **Mitigation**: 
  - Max iterations guard prevents infinite loops
  - Tool errors returned as results, don't crash loop
  - Pause only between tool calls, not mid-execution
  - Large results can be truncated in UI, full content in DB
