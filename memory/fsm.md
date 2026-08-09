# Finite State Machines (FSM)

## Purpose

This file documents the FSM-driven architecture for the chat tool loop, replacing the previous monolithic async implementation with a synchronous state machine that serves as the source of truth for messages, with external listeners for DB synchronization and extensibility.

## Overview

The FSM system consists of:
- **ToolLoopFsm**: Core state machine managing the tool loop lifecycle
- **TodoListFsm**: Helper FSM intercepting `rhd_set_todo_list` tool calls
- **RolesFsm**: Helper FSM intercepting `rhd_set_role` tool calls
- **BuiltinFsmManager**: Coordinator for helper FSMs
- **FsmToolLoop**: Async wrapper that drives the FSM and handles I/O
- **DB Sync Listener**: Synchronizes FSM state to database

## Architecture Decision

**Why FSM?**
- **Testability**: FSM can be tested deterministically without async
- **Separation of Concerns**: FSM handles state, async wrapper handles I/O
- **Extensibility**: Listener system allows plugins, helper FSMs
- **Source of Truth**: FSM maintains internal messages, DB synchronized via listeners

## ToolLoopFsm

**Location**: [`packages/rhd_fsm/src/tool_loop_fsm/mod.rs`](packages/rhd_fsm/src/tool_loop_fsm/mod.rs:22)

### States

```rust
pub enum State {
    Idle,
    AwaitingAiResponse { sent_messages: Vec<ChatMessage> },
    AwaitingToolResults { 
        pending_tool_calls: Vec<PendingToolCall>,
        collected_results: Vec<ToolResult>,
    },
    Paused { 
        pending_tool_calls: Vec<PendingToolCall>,
        collected_results: Vec<ToolResult>,
    },
    Aborted,
    Completed { message_id: i64 },
}
```

**Terminal states**: `Aborted`, `Completed`

### Inputs

```rust
pub enum ToolLoopInput {
    Run,
    InsertMessage { message: ChatMessage },
    RemoveMessage { message_id: i64 },
    ReplaceMessage { message_id: i64, new_message: ChatMessage },
    ReplaceAllMessages { messages: Vec<ChatMessage> },
    AddTool { tool: ToolDefinition },
    RemoveTool { tool_name: String },
    ProvideAiResponse {
        content: Option<String>,
        thinking_content: Option<String>,
        tool_calls: Vec<ToolCall>,
        finish_reason: String,
    },
    RequestToolCallId,
    ProvideToolResult { tool_call_id: String, result: ToolResult },
    Pause,
    Abort,
    Resume,
}
```

### Actions

```rust
pub enum ToolLoopAction {
    SendToAi { messages: Vec<ChatMessage>, tools: Vec<ToolDefinition> },
    ExecuteToolCall { tool_call: ToolCall },
    Completed { message: ChatMessage },
    Paused,
    Aborted,
    GenerateToolCallId { tool_call_id: String },
    Error { message: String },
}
```

### Events

```rust
pub enum ToolLoopFsmEvent {
    MessageInserted { message: ChatMessage },
    MessageRemoved { message_id: i64 },
    MessageReplaced { message_id: i64, new_message: ChatMessage },
    AllMessagesReplaced { messages: Vec<ChatMessage> },
    ToolCallRequested { tool_call: ToolCall, propagate: Arc<AtomicBool> },
    ToolCallExecuted { tool_call: ToolCall, result: ToolResult },
    AiResponseReceived { 
        content: Option<String>, 
        thinking_content: Option<String>, 
        tool_calls: Vec<ToolCall> 
    },
    ToolCallIdGenerated { tool_call_id: String },
    StateChanged { from: State, to: State },
}
```

### Listener System

**Location**: [`packages/rhd_fsm/src/tool_loop_fsm/listener.rs`](packages/rhd_fsm/src/tool_loop_fsm/listener.rs:13)

```rust
pub type ToolLoopListenerCallback = Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync>;

pub struct ToolLoopListenerManager {
    listeners: Vec<(ToolLoopListenerId, ToolLoopListenerCallback)>,
    next_id: usize,
}
```

**Methods**:
- `add_listener(callback) -> ToolLoopListenerId`
- `remove_listener(id) -> bool`
- `emit(event)` - calls all listeners synchronously

**Event Interception**: `ToolCallRequested` includes `propagate: Arc<AtomicBool>`. Listeners can set this to `false` to prevent the tool call from being executed by the async wrapper. The FSM checks this flag after emitting the event and skips `ExecuteToolCall` action if false.

### Message ID Generation

FSM maintains `message_id_counter: i64` field. Constructor accepts initial value via `with_message_id_counter(initial: i64)`. Each call to `generate_message_id()` increments and returns the counter.

### State Transitions

```
Idle → (Run) → AwaitingAiResponse
AwaitingAiResponse → (ProvideAiResponse with tool_calls) → AwaitingToolResults
AwaitingAiResponse → (ProvideAiResponse without tool_calls) → Completed
AwaitingAiResponse → (Pause) → Paused
AwaitingToolResults → (all results collected) → AwaitingAiResponse
AwaitingToolResults → (Pause) → Paused
Paused → (Resume) → AwaitingAiResponse or AwaitingToolResults
Any non-terminal → (Abort) → Aborted
```

## Helper FSMs

### TodoListFsm

**Location**: [`packages/rhd_fsm/src/todo_list_fsm.rs`](packages/rhd_fsm/src/todo_list_fsm.rs:16)

**Purpose**: Intercept `rhd_set_todo_list` tool calls and manage todo list state.

**State**:
```rust
pub enum TodoListState {
    Empty,
    Active { todos: String },
}
```

**Interception**: Creates a listener that checks if `tool_call.name == "rhd_set_todo_list"` and sets `propagate = false`.

**Handling**: `handle_set_todo_list(arguments)` parses JSON arguments, updates state, returns success message.

**Integration**: Registered with `ToolLoopFsm` via `register()` method. The async wrapper checks `is_intercepted(tool_call_id)` and calls `handle_builtin_tool()` instead of executing via MCP.

### RolesFsm

**Location**: [`packages/rhd_fsm/src/roles_fsm.rs`](packages/rhd_fsm/src/roles_fsm.rs:16)

**Purpose**: Intercept `rhd_set_role` tool calls and manage role state.

**State**:
```rust
pub enum RoleState {
    None,
    Active { role_name: String },
}
```

**Interception**: Same pattern as TodoListFsm, checks for `rhd_set_role` tool name.

**Handling**: `handle_set_role(arguments)` parses JSON, updates state, returns success message.

### BuiltinFsmManager

**Location**: [`packages/rhd_chat/src/tools/builtin_fsms.rs`](packages/rhd_chat/src/tools/builtin_fsms.rs:19)

**Purpose**: Coordinate helper FSMs and delegate builtin tool handling.

**Methods**:
- `register(tool_loop_fsm)` - registers both helper FSMs as listeners
- `unregister(tool_loop_fsm)` - removes both listeners
- `is_tool_intercepted(tool_call_id) -> bool` - checks if either FSM intercepted
- `handle_builtin_tool(manager, chat_id, tool_call, event_sender)` - delegates to appropriate handler

**Flow**:
1. Helper FSMs intercept tool calls via listeners, setting `propagate = false`
2. Async wrapper checks `is_tool_intercepted()` after receiving tool calls from AI
3. If intercepted, calls `handle_builtin_tool()` which:
   - Updates helper FSM state
   - Calls actual handler (`handle_rhd_set_todo_list` or `handle_rhd_set_role`)
   - Returns result to FSM via `ProvideToolResult` input

## Async Wrapper (FsmToolLoop)

**Location**: [`packages/rhd_chat/src/tools/fsm_wrapper.rs`](packages/rhd_chat/src/tools/fsm_wrapper.rs:25)

**Purpose**: Drive the FSM, handle I/O (AI calls, tool executions), manage pause/resume/abort.

**Structure**:
```rust
pub struct FsmToolLoop<'a, P: ProjectProvider> {
    fsm: ToolLoopFsm,
    builtin_fsm_manager: BuiltinFsmManager,
    manager: &'a ChatManager<P>,
    chat_id: i64,
    model: String,
    api_model: String,
    client: &'a OpenAiClient,
    mcp_clients: Vec<(String, String, Arc<McpClient>)>,
    cancel_token: CancellationToken,
    event_sender: broadcast::Sender<ChatEvent>,
    pause_notify: Arc<Notify>,
    iterations: u32,
    max_iterations: u32,
    loggers: Option<ChatLoggers>,
    template_loader: &'a TemplateLoaderRef,
}
```

**Main Loop** ([`run()`](packages/rhd_chat/src/tools/fsm_wrapper.rs:93)):
1. Load initial messages from DB into FSM
2. Load tools from projects into FSM
3. Feed `Run` input to FSM
4. Loop:
   - Check cancel token (abort if cancelled)
   - Check max iterations
   - Process actions:
     - `SendToAi`: Call AI client, feed `ProvideAiResponse` back
     - `ExecuteToolCall`: Execute tool, feed `ProvideToolResult` back
     - `Paused`: Wait on `pause_notify`, feed `Resume` on wake
     - `Completed`/`Aborted`: Exit loop
   - Handle intercepted builtin tools (check `is_tool_intercepted()`, call `handle_builtin_tool()`)

**Key Methods**:
- `load_initial_messages()` - loads DB messages into FSM
- `load_tools()` - loads tools from projects into FSM
- `handle_send_to_ai()` - calls AI client with streaming, accumulates content
- `handle_execute_tool_call()` - executes MCP tool, returns result
- `handle_builtin_tool()` - handles intercepted builtin tools
- `handle_abort()` - cleanup on abort
- `add_listener()` - exposes listener registration for DB sync

## DB Sync Listener

**Location**: [`packages/rhd_chat/src/tools/db_sync_listener.rs`](packages/rhd_chat/src/tools/db_sync_listener.rs:24)

**Purpose**: Keep database synchronized with FSM state.

**Function**: `create_db_sync_listener(db, chat_id, event_sender) -> ToolLoopListenerCallback`

**Event Handling**:
- `MessageInserted` → `db.insert_message()`, emit `ChatEvent::MessageAdded`
- `MessageRemoved` → `db.delete_message()`, emit `ChatEvent::MessageRemoved`
- `MessageReplaced` → `db.update_message_full()`, emit `ChatEvent::MessageReplaced`
- `AllMessagesReplaced` → `db.delete_all_messages()`, insert all, emit events
- Other events → no-op (handled by async wrapper)

**Message Conversion**: [`convert_fsm_message_to_db()`](packages/rhd_chat/src/tools/db_sync_listener.rs:170) converts FSM `ChatMessage` to DB `Message`, mapping tool calls to DB format.

## Integration Flow

### Normal Tool Loop

1. `tool_loop()` creates `FsmToolLoop` with DB sync listener
2. `FsmToolLoop::new()` initializes FSM, registers helper FSMs
3. `run()` loads messages and tools into FSM
4. FSM transitions: `Idle` → `AwaitingAiResponse`
5. Async wrapper calls AI, receives response
6. If tool calls:
   - Helper FSMs intercept builtin tools (set `propagate = false`)
   - FSM emits `ExecuteToolCall` for non-intercepted tools
   - Async wrapper executes tools, feeds results back
   - FSM transitions back to `AwaitingAiResponse`
7. If no tool calls: FSM transitions to `Completed`
8. DB sync listener keeps database in sync throughout

### Pause/Resume

1. User triggers pause
2. Async wrapper feeds `Pause` input to FSM
3. FSM transitions to `Paused`, emits `Paused` action
4. Async wrapper waits on `pause_notify.notified()`
5. User triggers resume
6. Async wrapper feeds `Resume` input to FSM
7. FSM transitions back to `AwaitingAiResponse` or `AwaitingToolResults`

### Abort

1. User triggers abort or cancel token is cancelled
2. Async wrapper feeds `Abort` input to FSM
3. FSM transitions to `Aborted`, emits `Aborted` action
4. Async wrapper calls `handle_abort()`, returns error

## Testing

### Unit Tests

**FSM Tests**: [`packages/rhd_fsm/src/tool_loop_fsm/tests.rs`](packages/rhd_fsm/src/tool_loop_fsm/tests.rs:1) (597 lines)
- State transitions, message/tool management, pause/resume/abort
- Listener system tests in `listener_tests` module

**Helper FSM Tests**: 
- [`todo_list_fsm.rs`](packages/rhd_fsm/src/todo_list_fsm.rs:134) - 4 tests
- [`roles_fsm.rs`](packages/rhd_fsm/src/roles_fsm.rs:134) - 4 tests
- [`builtin_fsms.rs`](packages/rhd_chat/src/tools/builtin_fsms.rs:112) - 2 tests

### Integration Tests

**Location**: [`packages/rhd_chat/src/tools/tests/fsm_integration_tests.rs`](packages/rhd_chat/src/tools/tests/fsm_integration_tests.rs:1) (576 lines, 15 tests)

**Coverage**:
- DB sync listener: message insert/remove/replace, tool calls, rapid events, special characters, long content
- FSM listeners: add/remove, multiple listeners, state changed events, tool call ID generation, event order

## Known Issues

1. **`generate_message_id()` visibility**: Method is private but tests call it directly. Needs `pub(crate)` or public accessor.

2. **`listener_manager` field visibility**: Tests access `fsm.listener_manager.listener_count()` but field is private. Needs `pub(crate)` or public method.

3. **Intercepted tool call tracking**: Helper FSM listeners track intercepted calls in local `Arc<Mutex<Vec>>`, but this is separate from `intercepted_tool_calls` field on FSM struct. The `is_intercepted()` check may not see calls intercepted by listeners. Needs sync mechanism or redesign.

## Related Files

- [`memory/chat.md`](memory/chat.md) - ChatManager, chat events, tool loop streaming
- [`memory/architecture.md`](memory/architecture.md) - Crate structure, component APIs
- [`plans/fsm-tool-loop-integration-grand-plan.md`](plans/fsm-tool-loop-integration-grand-plan.md) - Implementation plan
