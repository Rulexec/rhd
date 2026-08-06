# RHD FSM Package - ToolLoopFsm Implementation Plan

## Overview

Create a new `rhd_fsm` package that provides a pure synchronous finite state machine for managing the tool loop logic. This separates the state machine logic from async operations, making it easier to test and maintain.

## Architecture

### Package Structure

```
packages/rhd_fsm/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API exports
    ├── tool_loop_fsm.rs # Main FSM implementation
    ├── state.rs         # State definitions
    ├── input.rs         # Input types
    └── action.rs        # Action types
```

### Core Design

The FSM follows the pattern from `diffbelt_transforms/src/map_filter/mod.rs`:
- Single `run(&mut self, inputs: &mut Vec<Input>) -> Result<Actions, Error>` method
- Pure synchronous state transitions
- No async operations inside the FSM
- External code handles AI calls and tool execution, feeding results back

## State Machine Design

### States

```rust
pub enum State {
    /// Initial state, ready to start
    Idle,
    /// AI call in progress, waiting for response
    AwaitingAiResponse {
        /// Messages that were sent to AI
        sent_messages: Vec<ChatMessage>,
    },
    /// Tool calls received, waiting for results
    AwaitingToolResults {
        /// Pending tool calls that need results
        pending_tool_calls: Vec<PendingToolCall>,
        /// Collected tool results so far
        collected_results: Vec<ToolResult>,
    },
    /// Paused by user, can be resumed
    Paused {
        /// Pending tool calls if paused during tool execution
        pending_tool_calls: Vec<PendingToolCall>,
        /// Collected results before pause
        collected_results: Vec<ToolResult>,
    },
    /// Aborted, terminal state
    Aborted,
    /// Completed successfully, terminal state
    Completed {
        /// Final message ID
        message_id: i64,
    },
}
```

### Inputs

```rust
pub enum ToolLoopInput {
    /// Start/resume the tool loop
    Run,
    
    /// Insert a chat message
    InsertMessage { message: ChatMessage },
    
    /// Remove a chat message by ID
    RemoveMessage { message_id: i64 },
    
    /// Replace a chat message
    ReplaceMessage { message_id: i64, new_message: ChatMessage },
    
    /// Add available tool definition
    AddTool { tool: ToolDefinition },
    
    /// Remove available tool by name
    RemoveTool { tool_name: String },
    
    /// Provide AI response (after Run action)
    ProvideAiResponse {
        /// Content from AI
        content: Option<String>,
        /// Thinking/reasoning content
        thinking_content: Option<String>,
        /// Tool calls from AI (empty if final response)
        tool_calls: Vec<ToolCall>,
        /// Finish reason
        finish_reason: String,
    },
    
    /// Request next tool call ID
    RequestToolCallId,
    
    /// Provide tool call result
    ProvideToolResult {
        tool_call_id: String,
        result: ToolResult,
    },
    
    /// Pause the tool loop
    Pause,
    
    /// Abort the tool loop
    Abort,
    
    /// Resume from paused state
    Resume,
}
```

### Actions

```rust
pub enum ToolLoopAction {
    /// Send messages to AI and wait for response
    SendToAi {
        /// Messages to send
        messages: Vec<ChatMessage>,
        /// Available tools
        tools: Vec<ToolDefinition>,
    },
    
    /// Execute a tool call
    ExecuteToolCall {
        /// Tool call to execute
        tool_call: ToolCall,
    },
    
    /// Tool loop completed successfully
    Completed {
        /// Final assistant message
        message: ChatMessage,
    },
    
    /// Tool loop paused
    Paused,
    
    /// Tool loop aborted
    Aborted,
    
    /// Request a new unique tool call ID
    GenerateToolCallId,
    
    /// Error occurred
    Error {
        message: String,
    },
}
```

### FSM Structure

```rust
pub struct ToolLoopFsm {
    state: State,
    /// Current chat messages
    messages: Vec<ChatMessage>,
    /// Available tools
    tools: Vec<ToolDefinition>,
    /// Counter for generating unique tool call IDs
    tool_call_id_counter: u64,
}
```

## Implementation Details

### Key Methods

```rust
impl ToolLoopFsm {
    /// Create new FSM with initial configuration
    pub fn new() -> Self;
    
    /// Main entry point - process inputs and return actions
    pub fn run(&mut self, inputs: &mut Vec<ToolLoopInput>) -> Result<Vec<ToolLoopAction>, FsmError>;
    
    /// Get current state
    pub fn state(&self) -> &State;
    
    /// Check if FSM is in terminal state
    pub fn is_finished(&self) -> bool;
    
    /// Get current messages
    pub fn messages(&self) -> &[ChatMessage];
    
    /// Get available tools
    pub fn tools(&self) -> &[ToolDefinition];
}
```

### State Transitions

1. **Idle → AwaitingAiResponse**: On `Run` input
   - Action: `SendToAi` with current messages and tools

2. **AwaitingAiResponse → Completed**: On `ProvideAiResponse` with no tool calls
   - Action: `Completed` with final message

3. **AwaitingAiResponse → AwaitingToolResults**: On `ProvideAiResponse` with tool calls
   - Action: `ExecuteToolCall` for each tool call

4. **AwaitingToolResults → AwaitingAiResponse**: On `ProvideToolResult` when all results collected
   - Action: `SendToAi` with updated messages

5. **AwaitingToolResults → Paused**: On `Pause`
   - Action: `Paused`

6. **Paused → AwaitingAiResponse**: On `Resume`
   - Action: `SendToAi` or `ExecuteToolCall` depending on state

7. **Any → Aborted**: On `Abort`
   - Action: `Aborted`

### Tool Call ID Generation

The FSM maintains an internal counter for generating unique tool call IDs:
- `RequestToolCallId` input returns `GenerateToolCallId` action
- External code generates the actual ID and provides it back
- Or FSM generates IDs internally using the counter

### Message Management

The FSM maintains its own copy of messages:
- `InsertMessage` adds to internal list
- `RemoveMessage` removes from internal list
- `ReplaceMessage` updates in internal list
- Messages are sent to AI via `SendToAi` action

## Testing Strategy

1. **Unit tests** for each state transition
2. **Integration tests** for complete tool loop scenarios
3. **Edge cases**:
   - Pause during AI call
   - Pause during tool execution
   - Abort during AI call
   - Abort during tool execution
   - Resume from pause
   - Empty tool calls
   - Multiple tool calls

## Integration Plan (Future)

After implementation, the FSM will be integrated into `rhd_chat`:
1. Replace async `tool_loop()` function with FSM-based approach
2. `ChatManager` will create `ToolLoopFsm` instance per chat
3. Async operations (AI calls, tool execution) will be handled outside FSM
4. Results fed back to FSM via inputs
5. FSM emits actions that drive the async operations

## Files to Create

1. `packages/rhd_fsm/Cargo.toml` - Package manifest
2. `packages/rhd_fsm/src/lib.rs` - Public API
3. `packages/rhd_fsm/src/tool_loop_fsm.rs` - Main FSM implementation
4. `packages/rhd_fsm/src/state.rs` - State definitions
5. `packages/rhd_fsm/src/input.rs` - Input types
6. `packages/rhd_fsm/src/action.rs` - Action types
7. `packages/rhd_fsm/src/error.rs` - Error types
8. `packages/rhd_fsm/src/tests.rs` - Unit tests

## Dependencies

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

No async dependencies - pure synchronous FSM.
