# Module Split Refactoring Plan

## Goal
Split large monolithic files into smaller, focused modules to improve code organization and maintainability.

## Target Files

### 1. `packages/rhd_test/src/main.rs` (882 lines)

**Current Structure:**
- CLI argument parsing (Args struct)
- Mock server types (ChatRequest, ChatMessage, ToolCall, ChatResponse, etc.)
- Mock server implementation (chat_completions handler, start_mock_server)
- Test utilities (generate_random_string, create_temp_script)
- Two main test functions: `run_single_test` and `run_mcp_test`
- Main orchestration function

**Proposed Module Structure:**
```
packages/rhd_test/src/
├── main.rs           # Entry point, orchestration only
├── args.rs           # CLI argument definitions
├── mock_server.rs    # Mock AI server types and implementation
├── utils.rs          # Helper functions (generate_random_string, create_temp_script)
├── standard_test.rs  # run_single_test implementation
└── mcp_test.rs       # run_mcp_test implementation
```

**Module Responsibilities:**

- **`args.rs`**: Contains `Args` struct with clap definitions
- **`mock_server.rs`**: All mock server types (ChatRequest, ChatMessage, ToolCall, ChatResponse, Choice, ResponseMessage, ToolCallResponse, FunctionCallResponse, RecordedRequest) and functions (chat_completions, start_mock_server)
- **`utils.rs`**: Helper functions `generate_random_string` and `create_temp_script`
- **`standard_test.rs`**: `run_single_test` function and related logic
- **`mcp_test.rs`**: `run_mcp_test` function and related logic
- **`main.rs`**: Simplified to only contain `main` function that orchestrates test execution

### 2. `packages/rhd_app/src/scenario/executor.rs` (520 lines)

**Current Structure:**
- Error types (ExecuteError enum)
- ExecuteOutput struct
- Main `execute_scenario` function
- `evaluate_skip` helper
- `execute_run_command` function (120 lines)
- `execute_ai_chat` function (100 lines)
- `execute_ai_chat_with_tools` function (170 lines)

**Proposed Module Structure:**
```
packages/rhd_app/src/scenario/
├── mod.rs            # Re-exports, Action/Scenario structs (unchanged)
├── loader.rs         # YAML loading (unchanged)
├── placeholder.rs    # Placeholder resolution (unchanged)
├── error.rs          # ExecuteError enum
├── run_command.rs    # execute_run_command function
└── ai_chat.rs        # execute_ai_chat and execute_ai_chat_with_tools functions
```

**Module Responsibilities:**

- **`error.rs`**: `ExecuteError` enum and `ExecuteOutput` struct
- **`run_command.rs`**: `execute_run_command` function
- **`ai_chat.rs`**: `execute_ai_chat` and `execute_ai_chat_with_tools` functions
- **`executor.rs`** (simplified): Main `execute_scenario` function that delegates to sub-modules

## Implementation Steps

### Phase 1: Refactor `rhd_test`

1. Create `args.rs` module
   - Move `Args` struct and clap definitions
   - Update `main.rs` to import from `args`

2. Create `mock_server.rs` module
   - Move all mock server types (ChatRequest, ChatMessage, etc.)
   - Move `chat_completions` handler
   - Move `start_mock_server` function
   - Move type aliases (SharedRequests, SharedResponse, SharedFlagValue)
   - Update `main.rs` to import from `mock_server`

3. Create `utils.rs` module
   - Move `generate_random_string` function
   - Move `create_temp_script` function
   - Update `main.rs` to import from `utils`

4. Create `standard_test.rs` module
   - Move `run_single_test` function
   - Update `main.rs` to import from `standard_test`

5. Create `mcp_test.rs` module
   - Move `run_mcp_test` function
   - Update `main.rs` to import from `mcp_test`

6. Simplify `main.rs`
   - Keep only `main` function
   - Import all necessary items from new modules
   - Verify compilation and test execution

### Phase 2: Refactor `rhd_app/scenario/executor.rs`

1. Create `error.rs` module
   - Move `ExecuteError` enum
   - Move `ExecuteOutput` struct
   - Move `From<rhd_ai::AiError>` implementation
   - Update `executor.rs` to import from `error`

2. Create `run_command.rs` module
   - Move `execute_run_command` function
   - Update `executor.rs` to import from `run_command`

3. Create `ai_chat.rs` module
   - Move `execute_ai_chat` function
   - Move `execute_ai_chat_with_tools` function
   - Update `executor.rs` to import from `ai_chat`

4. Simplify `executor.rs`
   - Keep `execute_scenario` and `evaluate_skip` functions
   - Import error types and execution functions from sub-modules
   - Verify compilation

### Phase 3: Validation

1. Run `cargo build` to verify compilation
2. Run `cargo test` to verify unit tests pass
3. Run `cargo run -p rhd_test` to verify E2E tests pass
4. Review code organization and module boundaries

## Risks and Mitigations

**Risk**: Circular dependencies between modules
**Mitigation**: Careful planning of module boundaries; use `mod.rs` for re-exports

**Risk**: Breaking existing functionality
**Mitigation**: Comprehensive testing after each phase; incremental refactoring

**Risk**: Increased complexity from too many small modules
**Mitigation**: Balance between cohesion and file count; each module should have clear responsibility

## Success Criteria

- All files under 300 lines (except `mod.rs` files which may be larger due to struct definitions)
- Clear module boundaries with single responsibilities
- All tests pass (unit tests and E2E tests)
- No circular dependencies
- Code compiles without warnings
