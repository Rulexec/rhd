# FSM Debug Logging Plan

## Overview

Add comprehensive debug logging to all FSMs in the system using the `tracing` crate. This will log every input fed into FSMs and all outputs they emit, along with formatted internal state information.

## FSMs to Instrument

1. **ToolLoopFsm** (`packages/rhd_fsm/src/tool_loop_fsm/mod.rs`)
   - Core state machine for the chat tool loop
   - States: Idle, AwaitingAiResponse, AwaitingToolResults, Paused, Aborted, Completed

2. **StreamLifecycleFsm** (`packages/rhd_chat/src/stream_fsm.rs`)
   - Manages stream lifecycle (register, pause, resume, abort)
   - States: Idle, Running, Paused, Aborted
   - Already has some tracing, needs enhancement

3. **TodoListFsm** (`packages/rhd_fsm/src/todo_list_fsm.rs`)
   - Helper FSM for intercepting `rhd_set_todo_list` tool calls
   - States: Empty, Active { todos: String }

4. **RolesFsm** (`packages/rhd_fsm/src/roles_fsm.rs`)
   - Helper FSM for intercepting `rhd_set_role` tool calls
   - States: None, Active { role_name: String }

## Implementation Plan

### Phase 1: Add Display Implementations for States

Add `Display` trait implementations for all state enums that format the state name and key values, omitting serialization of large maps/lists.

#### 1.1 ToolLoopFsm State Display

**File:** `packages/rhd_fsm/src/tool_loop_fsm/state.rs`

Add `Display` implementation for `State`:

```rust
impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::AwaitingAiResponse { sent_messages } => {
                write!(f, "AwaitingAiResponse {{ sent_messages_count: {} }}", sent_messages.len())
            }
            State::AwaitingToolResults { pending_tool_calls, collected_results } => {
                write!(f, "AwaitingToolResults {{ pending: {}, collected: {} }}", 
                    pending_tool_calls.len(), collected_results.len())
            }
            State::Paused { pending_tool_calls, collected_results } => {
                write!(f, "Paused {{ pending: {}, collected: {} }}", 
                    pending_tool_calls.len(), collected_results.len())
            }
            State::Aborted => write!(f, "Aborted"),
            State::Completed { message_id } => {
                write!(f, "Completed {{ message_id: {} }}", message_id)
            }
        }
    }
}
```

#### 1.2 StreamLifecycleFsm State Display

**File:** `packages/rhd_chat/src/stream_fsm.rs`

Add `Display` implementation for `StreamLifecycleState`:

```rust
impl std::fmt::Display for StreamLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleState::Idle => write!(f, "Idle"),
            StreamLifecycleState::Running { phase, .. } => {
                write!(f, "Running {{ phase: {:?} }}", phase)
            }
            StreamLifecycleState::Paused { phase, message_queue, .. } => {
                write!(f, "Paused {{ phase: {:?}, queue_size: {} }}", 
                    phase, message_queue.len())
            }
            StreamLifecycleState::Aborted { aborted_tool_ids, message_queue, .. } => {
                write!(f, "Aborted {{ aborted_tools: {}, queue_size: {} }}", 
                    aborted_tool_ids.len(), message_queue.len())
            }
        }
    }
}
```

#### 1.3 TodoListFsm State Display

**File:** `packages/rhd_fsm/src/todo_list_fsm.rs`

Add `Display` implementation for `TodoListState`:

```rust
impl std::fmt::Display for TodoListState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoListState::Empty => write!(f, "Empty"),
            TodoListState::Active { todos } => {
                write!(f, "Active {{ todos_length: {} }}", todos.len())
            }
        }
    }
}
```

#### 1.4 RolesFsm State Display

**File:** `packages/rhd_fsm/src/roles_fsm.rs`

Add `Display` implementation for `RoleState`:

```rust
impl std::fmt::Display for RoleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoleState::None => write!(f, "None"),
            RoleState::Active { role_name } => {
                write!(f, "Active {{ role_name: {} }}", role_name)
            }
        }
    }
}
```

### Phase 2: Add Input/Output Display Implementations

Add `Display` implementations for input and action enums to provide human-readable logging.

#### 2.1 ToolLoopInput Display

**File:** `packages/rhd_fsm/src/tool_loop_fsm/input.rs`

```rust
impl std::fmt::Display for ToolLoopInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolLoopInput::Run => write!(f, "Run"),
            ToolLoopInput::InsertMessage { message } => {
                write!(f, "InsertMessage {{ id: {}, role: {} }}", message.id, message.role)
            }
            ToolLoopInput::RemoveMessage { message_id } => {
                write!(f, "RemoveMessage {{ message_id: {} }}", message_id)
            }
            ToolLoopInput::ReplaceMessage { message_id, new_message } => {
                write!(f, "ReplaceMessage {{ message_id: {}, new_role: {} }}", 
                    message_id, new_message.role)
            }
            ToolLoopInput::ReplaceAllMessages { messages } => {
                write!(f, "ReplaceAllMessages {{ count: {} }}", messages.len())
            }
            ToolLoopInput::AddTool { tool } => {
                write!(f, "AddTool {{ name: {} }}", tool.name)
            }
            ToolLoopInput::RemoveTool { tool_name } => {
                write!(f, "RemoveTool {{ name: {} }}", tool_name)
            }
            ToolLoopInput::ProvideAiResponse { content, tool_calls, finish_reason, .. } => {
                write!(f, "ProvideAiResponse {{ has_content: {}, tool_calls: {}, finish_reason: {} }}", 
                    content.is_some(), tool_calls.len(), finish_reason)
            }
            ToolLoopInput::RequestToolCallId => write!(f, "RequestToolCallId"),
            ToolLoopInput::ProvideToolResult { tool_call_id, result } => {
                write!(f, "ProvideToolResult {{ tool_call_id: {}, is_error: {} }}", 
                    tool_call_id, result.is_error)
            }
            ToolLoopInput::Pause => write!(f, "Pause"),
            ToolLoopInput::Abort => write!(f, "Abort"),
            ToolLoopInput::Resume => write!(f, "Resume"),
        }
    }
}
```

#### 2.2 ToolLoopAction Display

**File:** `packages/rhd_fsm/src/tool_loop_fsm/action.rs`

```rust
impl std::fmt::Display for ToolLoopAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolLoopAction::SendToAi { messages, tools } => {
                write!(f, "SendToAi {{ messages: {}, tools: {} }}", messages.len(), tools.len())
            }
            ToolLoopAction::ExecuteToolCall { tool_call } => {
                write!(f, "ExecuteToolCall {{ name: {}, id: {} }}", tool_call.name, tool_call.id)
            }
            ToolLoopAction::Completed { message } => {
                write!(f, "Completed {{ message_id: {} }}", message.id)
            }
            ToolLoopAction::Paused => write!(f, "Paused"),
            ToolLoopAction::Aborted => write!(f, "Aborted"),
            ToolLoopAction::GenerateToolCallId { tool_call_id } => {
                write!(f, "GenerateToolCallId {{ id: {} }}", tool_call_id)
            }
            ToolLoopAction::Error { message } => {
                write!(f, "Error {{ message: {} }}", message)
            }
        }
    }
}
```

#### 2.3 StreamLifecycleInput Display

**File:** `packages/rhd_chat/src/stream_fsm.rs`

```rust
impl std::fmt::Display for StreamLifecycleInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleInput::Register { .. } => write!(f, "Register"),
            StreamLifecycleInput::Pause => write!(f, "Pause"),
            StreamLifecycleInput::Resume => write!(f, "Resume"),
            StreamLifecycleInput::Abort { aborted_tool_ids } => {
                write!(f, "Abort {{ aborted_tools: {} }}", aborted_tool_ids.len())
            }
            StreamLifecycleInput::SetPhase { phase } => {
                write!(f, "SetPhase {{ phase: {:?} }}", phase)
            }
            StreamLifecycleInput::QueueMessage { message } => {
                write!(f, "QueueMessage {{ id: {} }}", message.id)
            }
            StreamLifecycleInput::Unregister => write!(f, "Unregister"),
        }
    }
}
```

#### 2.4 StreamLifecycleAction Display

**File:** `packages/rhd_chat/src/stream_fsm.rs`

```rust
impl std::fmt::Display for StreamLifecycleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleAction::Registered => write!(f, "Registered"),
            StreamLifecycleAction::Paused => write!(f, "Paused"),
            StreamLifecycleAction::Resumed => write!(f, "Resumed"),
            StreamLifecycleAction::Aborted => write!(f, "Aborted"),
            StreamLifecycleAction::Unregistered => write!(f, "Unregistered"),
            StreamLifecycleAction::PhaseChanged { phase } => {
                write!(f, "PhaseChanged {{ phase: {:?} }}", phase)
            }
            StreamLifecycleAction::MessageQueued => write!(f, "MessageQueued"),
            StreamLifecycleAction::Error { message } => {
                write!(f, "Error {{ message: {} }}", message)
            }
        }
    }
}
```

### Phase 3: Add Tracing to ToolLoopFsm

**File:** `packages/rhd_fsm/src/tool_loop_fsm/mod.rs`

Add tracing to the `run()` and `process_input()` methods:

```rust
pub fn run(&mut self, inputs: &mut Vec<ToolLoopInput>) -> Result<Vec<ToolLoopAction>, FsmError> {
    let mut actions = Vec::new();

    tracing::debug!(
        fsm = "ToolLoopFsm",
        state = %self.state,
        inputs_count = inputs.len(),
        "ToolLoopFsm::run started"
    );

    for input in inputs.drain(..) {
        tracing::debug!(
            fsm = "ToolLoopFsm",
            state_before = %self.state,
            input = %input,
            "Processing input"
        );

        let input_actions = self.process_input(input)?;
        
        for action in &input_actions {
            tracing::debug!(
                fsm = "ToolLoopFsm",
                state_after = %self.state,
                action = %action,
                "Emitted action"
            );
        }
        
        actions.extend(input_actions);
    }

    tracing::debug!(
        fsm = "ToolLoopFsm",
        final_state = %self.state,
        actions_count = actions.len(),
        "ToolLoopFsm::run completed"
    );

    Ok(actions)
}
```

### Phase 4: Enhance StreamLifecycleFsm Tracing

**File:** `packages/rhd_chat/src/stream_fsm.rs`

Enhance the existing tracing in `handle()` method to use the new Display implementations:

```rust
pub fn handle(&mut self, input: StreamLifecycleInput) -> Vec<StreamLifecycleAction> {
    let mut actions = Vec::new();

    tracing::info!(
        fsm = "StreamLifecycleFsm",
        chat_id = self.chat_id,
        state_before = %self.state,
        input = %input,
        "StreamLifecycleFsm::handle started"
    );

    // ... existing match logic ...

    for action in &actions {
        tracing::info!(
            fsm = "StreamLifecycleFsm",
            chat_id = self.chat_id,
            state_after = %self.state,
            action = %action,
            "Emitted action"
        );
    }

    actions
}
```

### Phase 5: Add Tracing to Helper FSMs

#### 5.1 TodoListFsm Tracing

**File:** `packages/rhd_fsm/src/todo_list_fsm.rs`

Add tracing to key methods:

```rust
pub fn handle_set_todo_list(&mut self, arguments: &str) -> String {
    tracing::debug!(
        fsm = "TodoListFsm",
        state_before = %self.state,
        arguments_length = arguments.len(),
        "TodoListFsm::handle_set_todo_list"
    );

    // ... existing logic ...

    tracing::debug!(
        fsm = "TodoListFsm",
        state_after = %self.state,
        "TodoListFsm state updated"
    );

    // ... return result ...
}
```

#### 5.2 RolesFsm Tracing

**File:** `packages/rhd_fsm/src/roles_fsm.rs`

Add tracing to key methods:

```rust
pub fn handle_set_role(&mut self, arguments: &str) -> String {
    tracing::debug!(
        fsm = "RolesFsm",
        state_before = %self.state,
        arguments_length = arguments.len(),
        "RolesFsm::handle_set_role"
    );

    // ... existing logic ...

    tracing::debug!(
        fsm = "RolesFsm",
        state_after = %self.state,
        "RolesFsm state updated"
    );

    // ... return result ...
}
```

### Phase 6: Add Tracing to FsmToolLoop

**File:** `packages/rhd_chat/src/tools/fsm_wrapper.rs`

Add tracing to the main loop and key methods:

```rust
pub async fn run(&mut self) -> Result<(), ChatError> {
    tracing::info!(
        chat_id = self.chat_id,
        model = %self.model,
        "FsmToolLoop::run started"
    );

    // ... existing logic ...

    loop {
        // Check cancel token
        if self.cancel_token.is_cancelled() {
            tracing::info!(
                chat_id = self.chat_id,
                "FsmToolLoop: cancel token triggered, aborting"
            );
            // ... abort logic ...
        }

        // Process actions
        for action in actions {
            tracing::debug!(
                chat_id = self.chat_id,
                fsm_state = %self.fsm.state(),
                action = ?action,
                "FsmToolLoop processing action"
            );

            // ... action handling ...
        }
    }

    tracing::info!(
        chat_id = self.chat_id,
        final_state = %self.fsm.state(),
        "FsmToolLoop::run completed"
    );

    Ok(())
}
```

## Implementation Checklist

- [ ] Add `Display` implementation for `State` in `tool_loop_fsm/state.rs`
- [ ] Add `Display` implementation for `ToolLoopInput` in `tool_loop_fsm/input.rs`
- [ ] Add `Display` implementation for `ToolLoopAction` in `tool_loop_fsm/action.rs`
- [ ] Add `Display` implementation for `StreamLifecycleState` in `stream_fsm.rs`
- [ ] Add `Display` implementation for `StreamLifecycleInput` in `stream_fsm.rs`
- [ ] Add `Display` implementation for `StreamLifecycleAction` in `stream_fsm.rs`
- [ ] Add `Display` implementation for `TodoListState` in `todo_list_fsm.rs`
- [ ] Add `Display` implementation for `RoleState` in `roles_fsm.rs`
- [ ] Add tracing to `ToolLoopFsm::run()` and `process_input()`
- [ ] Enhance tracing in `StreamLifecycleFsm::handle()`
- [ ] Add tracing to `TodoListFsm::handle_set_todo_list()`
- [ ] Add tracing to `RolesFsm::handle_set_role()`
- [ ] Add tracing to `FsmToolLoop::run()` and key methods
- [ ] Run tests to verify logging works correctly
- [ ] Verify log output format is readable and useful

## Log Output Examples

### ToolLoopFsm Input/Output

```
DEBUG ToolLoopFsm::run started state=Idle inputs_count=1
DEBUG Processing input state_before=Idle input=Run
DEBUG Emitted action state_after=AwaitingAiResponse { sent_messages_count: 5 } action=SendToAi { messages: 5, tools: 3 }
DEBUG ToolLoopFsm::run completed final_state=AwaitingAiResponse { sent_messages_count: 5 } actions_count=1
```

### StreamLifecycleFsm Input/Output

```
INFO StreamLifecycleFsm::handle started chat_id=123 state_before=Idle input=Register
INFO Emitted action chat_id=123 state_after=Running { phase: AiCall } action=Registered
```

### Helper FSM State Changes

```
DEBUG TodoListFsm::handle_set_todo_list state_before=Empty arguments_length=256
DEBUG TodoListFsm state updated state_after=Active { todos_length: 256 }
```

## Benefits

1. **Complete Visibility**: Every FSM input and output is logged
2. **State Tracking**: Current state is always visible in logs
3. **Debugging**: Easy to trace FSM behavior during test failures
4. **Monitoring**: Can monitor FSM health in production
5. **Readable**: Display implementations provide human-readable state summaries without overwhelming detail

## Risk Assessment

**Low Risk:**
- Adding Display implementations (purely additive)
- Adding tracing statements (minimal performance impact)

**Medium Risk:**
- Modifying FSM code (could introduce bugs)
- Log volume in production (need to ensure appropriate log levels)

**Mitigation:**
- Use `debug!` level for detailed FSM operations
- Use `info!` level for significant state changes
- Test with existing test suite to ensure no regressions
