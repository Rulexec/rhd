# FSM Conversion Candidates Analysis

## Goal
Rewrite stateful operations in `rhd_app` and `rhd_chat` packages to finite state machine style for:
- Single entry point for all logic
- Easier logging and debugging
- Better testability
- Clearer state transitions

## Current State Analysis

### rhd_app Package

#### 1. ExecutionTracker & ExecutionHandle (HIGH PRIORITY)
**Location**: `packages/rhd_app/src/execution/mod.rs`, `packages/rhd_app/src/execution/tracker.rs`

**Current Structure**:
- `ExecutionTracker`: Manages multiple active executions
- `ExecutionHandle`: Tracks individual execution state
- `ActiveExecution`: Holds scenario state with pause/resume channels
- `ExecutionState`: Tracks step timings, token usage, current step info

**State Transitions**:
```
Idle → (start) → Running
Running → (pause) → Paused
Paused → (resume) → Running
Paused → (abort) → Aborted
Running → (abort) → Aborted
Aborted → (cleanup) → Idle
```

**Why FSM Candidate**:
- ✅ Clear state machine semantics (running, paused, aborted)
- ✅ Multiple entry points (start, pause, resume, abort)
- ✅ State scattered across multiple fields (pause_state, resume_tx, abort_handle)
- ✅ Complex transition logic spread across methods
- ✅ Would benefit from centralized logging at single entry point
- ✅ Testable without async (like ToolLoopFsm)

**Current Pain Points**:
- State validation scattered across methods
- No explicit state transition validation
- Hard to trace execution flow for debugging
- Multiple mutex locks for different state aspects

---

### rhd_chat Package

#### 2. ChatManager Stream Operations (MEDIUM PRIORITY)
**Location**: `packages/rhd_chat/src/manager.rs`, `packages/rhd_chat/src/state.rs`

**Current Structure**:
- `ChatManager`: Manages active streams with `active_streams: Mutex<HashMap<i64, StreamState>>`
- `StreamState`: Enum with Running, Paused, Aborted variants
- Operations: `register_stream`, `unregister_stream`, `pause_stream`, `resume_stream`, `abort_stream`

**State Transitions**:
```
NoStream → (register) → Running
Running → (pause) → Paused
Paused → (resume) → Running
Paused → (abort) → Aborted
Running → (abort) → Aborted
Any → (unregister) → NoStream
```

**Why FSM Candidate**:
- ✅ Already has FSM-like enum structure
- ⚠️ Operations are methods on ChatManager, not centralized
- ⚠️ State transitions implicit in method calls
- ✅ Would benefit from explicit transition validation
- ✅ Single entry point would help with logging

**Current Pain Points**:
- Stream state management mixed with ChatManager responsibilities
- No explicit state transition validation
- Hard to track state changes for debugging

**Note**: The tool loop itself is already FSM-driven (ToolLoopFsm in rhd_fsm), so this is about the outer stream lifecycle management.

#### 3. MessageQueue (LOW PRIORITY - Part of #2)
**Location**: `packages/rhd_chat/src/state.rs`

**Current Structure**:
- `MessageQueue`: Stores messages queued during pause/abort
- `QueuedMessage`: Individual queued message

**Why Not Separate FSM**:
- MessageQueue is a data structure, not a state machine
- Should be part of ChatManager stream FSM as state data

---

## Recommended FSM Conversions

### Priority 1: ExecutionTracker FSM

**New Structure**:
```rust
// packages/rhd_app/src/execution/fsm.rs

pub enum ExecutionState {
    Idle,
    Running {
        scenario_name: String,
        started_at: DateTime<Utc>,
        step_timings: Vec<StepTiming>,
        token_usage: TokenUsage,
        current_step: Option<CurrentStep>,
    },
    Paused {
        scenario_name: String,
        started_at: DateTime<Utc>,
        pause_reason: String,
        step_name: String,
        step_timings: Vec<StepTiming>,
        token_usage: TokenUsage,
    },
    Aborted {
        scenario_name: String,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        step_timings: Vec<StepTiming>,
        token_usage: TokenUsage,
    },
}

pub enum ExecutionInput {
    Start { scenario_name: String },
    Pause { error: String, step_name: String },
    Resume { model_override: Option<String> },
    Abort,
    AddStepTiming(StepTiming),
    AddTokenUsage(TokenUsage),
    SetCurrentStep(CurrentStep),
    Finish,
}

pub enum ExecutionAction {
    EmitStarted { id: u64, scenario_name: String },
    EmitPaused { id: u64, error: String, step_name: String },
    EmitResumed { id: u64 },
    EmitAborted { id: u64 },
    EmitFinished { id: u64, meta: ScenarioMeta },
    WaitForResume,
    Error { message: String },
}

pub struct ExecutionFsm {
    id: u64,
    state: ExecutionState,
    abort_handle: AbortHandle,
    resume_tx: Option<oneshot::Sender<ResumeAction>>,
}

impl ExecutionFsm {
    pub fn new(id: u64) -> Self { ... }
    pub fn handle(&mut self, input: ExecutionInput) -> Vec<ExecutionAction> { ... }
    pub fn state(&self) -> &ExecutionState { ... }
}
```

**Benefits**:
- Single entry point: `handle(input)` method
- All state transitions in one place
- Easy to add logging at entry point
- Testable without async
- Clear state validation

**Integration**:
- `ExecutionTracker` becomes a coordinator that:
  - Maintains `HashMap<u64, ExecutionFsm>`
  - Drives FSMs based on external events
  - Handles I/O (database, events)
  - Similar to `FsmToolLoop` pattern

---

### Priority 2: ChatManager Stream FSM

**New Structure**:
```rust
// packages/rhd_chat/src/stream_fsm.rs

pub enum StreamLifecycleState {
    Idle,
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
        message_queue: MessageQueue,
    },
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
        message_queue: MessageQueue,
    },
}

pub enum StreamLifecycleInput {
    Register,
    Pause,
    Resume,
    Abort,
    SetPhase(ExecutionPhase),
    QueueMessage(QueuedMessage),
    Unregister,
}

pub enum StreamLifecycleAction {
    CreateCancelToken,
    WaitForResume,
    CancelOperations,
    Cleanup,
    Error { message: String },
}

pub struct StreamLifecycleFsm {
    chat_id: i64,
    state: StreamLifecycleState,
}

impl StreamLifecycleFsm {
    pub fn new(chat_id: i64) -> Self { ... }
    pub fn handle(&mut self, input: StreamLifecycleInput) -> Vec<StreamLifecycleAction> { ... }
    pub fn state(&self) -> &StreamLifecycleState { ... }
}
```

**Benefits**:
- Explicit state transitions
- Centralized stream lifecycle management
- Easier to debug stream state issues
- Can add logging at single entry point

**Integration**:
- `ChatManager` maintains `HashMap<i64, StreamLifecycleFsm>`
- Delegates stream operations to FSM
- Handles I/O (WebSocket events, database)

---

## Implementation Strategy

### Phase 1: ExecutionTracker FSM
1. Create `ExecutionFsm` in `packages/rhd_app/src/execution/fsm.rs`
2. Port state transition logic from `ExecutionTracker` methods
3. Add comprehensive unit tests (no async needed)
4. Refactor `ExecutionTracker` to use `ExecutionFsm`
5. Add logging at FSM entry point
6. Integration tests

### Phase 2: ChatManager Stream FSM
1. Create `StreamLifecycleFsm` in `packages/rhd_chat/src/stream_fsm.rs`
2. Port state transition logic from `ChatManager` stream methods
3. Add comprehensive unit tests
4. Refactor `ChatManager` to use `StreamLifecycleFsm`
5. Add logging at FSM entry point
6. Integration tests

---

## Comparison with Existing ToolLoopFsm

| Aspect | ToolLoopFsm | ExecutionFsm (proposed) | StreamLifecycleFsm (proposed) |
|--------|-------------|-------------------------|-------------------------------|
| Location | rhd_fsm | rhd_app | rhd_chat |
| Purpose | Tool loop lifecycle | Scenario execution lifecycle | Chat stream lifecycle |
| States | 5 (Idle, AwaitingAi, AwaitingTool, Paused, Aborted, Completed) | 4 (Idle, Running, Paused, Aborted) | 4 (Idle, Running, Paused, Aborted) |
| Async wrapper | FsmToolLoop | ExecutionTracker (refactored) | ChatManager (refactored) |
| Listener system | Yes | Could add | Could add |
| DB sync | Via listener | Direct (tracker owns DB) | Direct (manager owns DB) |

---

## Risks and Mitigations

### Risk 1: Breaking existing functionality
**Mitigation**: 
- Comprehensive integration tests before refactoring
- Gradual migration (FSM alongside existing code)
- Feature flags for gradual rollout

### Risk 2: Performance overhead
**Mitigation**:
- FSM is synchronous, minimal overhead
- No additional async operations
- Same number of state transitions

### Risk 3: Complexity increase
**Mitigation**:
- Clear separation of concerns (FSM = state, coordinator = I/O)
- Better testability offsets complexity
- Easier to understand state transitions

---

## Success Criteria

1. **Single Entry Point**: All state changes go through `handle(input)` method
2. **Logging**: Can add logging at FSM entry point to trace all state changes
3. **Testability**: FSM can be tested without async
4. **Maintainability**: State transitions are explicit and validated
5. **No Regressions**: All existing tests pass
6. **Performance**: No measurable performance degradation

---

## Next Steps

1. Review this analysis with team
2. Prioritize ExecutionTracker FSM (Phase 1)
3. Create detailed implementation plan for Phase 1
4. Implement and test ExecutionTracker FSM
5. Evaluate results before proceeding to Phase 2
