# Code Refactoring Plan

## Overview

This plan addresses two main refactoring goals:
1. Extract inline `#[cfg(test)]` modules to separate files
2. Refactor files over 400 lines by splitting them into logical modules

## Status

**Phase 1: COMPLETE** - All inline `#[cfg(test)]` modules have been extracted to separate files.

**Phase 2: COMPLETE** - All large files (>400 lines) have been split into logical modules.

### Completed in Phase 2:
- ✅ 2.1 `packages/rhd_chat/src/tools/mod.rs` (753 lines) → Split into `tools/` directory with `mod.rs`, `builtin.rs`, `tool_loop.rs`, `messages.rs`, `utils.rs`
- ✅ 2.2 `packages/rhd_ai/src/client.rs` (879 lines) → Split into `client/` directory with `mod.rs`, `types.rs`, `chat.rs`, `stream.rs`
- ✅ 2.3 `packages/rhd_app/src/ws.rs` (873 lines) → Split into `ws/` directory with `mod.rs`, `handlers.rs`, `events.rs`
- ✅ 2.4 `packages/rhd_db/src/chat_db/mod.rs` (620 lines) → Split into `chat_db/` directory with `mod.rs`, `schema.rs`, `chats.rs`, `messages.rs`, `projects.rs`
- ✅ 2.5 `packages/rhd_app/src/daemon.rs` (615 lines) → Split into `daemon/` directory with `mod.rs`, `run.rs`, `ipc.rs`, `reload.rs`
- ✅ 2.6 `packages/rhd_api/src/lib.rs` (593 lines) → Split into `execution.rs`, `chat.rs`, `ws.rs`
- ✅ 2.7 `packages/rhd_chat/src/stream.rs` (516 lines) → Split into `stream/` directory with `mod.rs`, `contract.rs`, `send.rs`
- ✅ 2.8 `packages/rhd_app/src/scenario/ai_chat.rs` (475 lines) → Split into `ai_chat/` directory with `mod.rs`, `simple.rs`, `mcp.rs`, `utils.rs`
- ✅ 2.9 `packages/rhd_app/src/execution.rs` (410 lines) → Split into `execution/` directory with `mod.rs`, `tracker.rs`, `handle.rs`

### Test Files to Split (>400 lines):
- ⏳ 2.10 `packages/rhd_chat/src/tools/tests.rs` (938 lines)
- ⏳ 2.11 `packages/rhd_db/src/chat_db/tests.rs` (503 lines)
- ⏳ 2.12 `packages/rhd_test/src/standard_test.rs` (444 lines)
- ⏳ 2.13 `packages/rhd_test/src/mock_server.rs` (415 lines)

## Current State After Phase 1

### Files Over 400 Lines (Requiring Phase 2 Splitting)

| File | Current Lines | Needs Splitting |
|------|---------------|-----------------|
| `packages/rhd_chat/src/tools/tests.rs` | 938 | Test file - no split needed |
| `packages/rhd_ai/src/client.rs` | 879 | Yes |
| `packages/rhd_app/src/ws.rs` | 873 | Yes |
| `packages/rhd_chat/src/tools/mod.rs` | 753 | Yes |
| `packages/rhd_db/src/chat_db/mod.rs` | 620 | Yes |
| `packages/rhd_app/src/daemon.rs` | 615 | Yes |
| `packages/rhd_api/src/lib.rs` | 593 | Yes |
| `packages/rhd_chat/src/stream.rs` | 516 | Yes |
| `packages/rhd_db/src/chat_db/tests.rs` | 503 | Test file - no split needed |
| `packages/rhd_app/src/scenario/ai_chat.rs` | 475 | Yes |
| `packages/rhd_test/src/standard_test.rs` | 444 | Test file - no split needed |
| `packages/rhd_test/src/mock_server.rs` | 415 | Test file - no split needed |
| `packages/rhd_app/src/execution.rs` | 410 | Yes |

### Files Under 400 Lines (No Action Needed)

All other files are under 400 lines and do not require splitting.

---

## Phase 1: Extract Inline Test Modules - COMPLETE

All inline `#[cfg(test)]` modules have been successfully extracted to separate files:

### Completed Extractions

| Original File | Test File Created |
|---------------|-------------------|
| `packages/rhd_chat/src/tools.rs` | `packages/rhd_chat/src/tools/tests.rs` |
| `packages/rhd_db/src/chat_db.rs` | `packages/rhd_db/src/chat_db/tests.rs` |
| `packages/rhd_api/src/lib.rs` | `packages/rhd_api/src/tests.rs` |
| `packages/rhd_app/src/ws.rs` | `packages/rhd_app/src/ws/tests.rs` |
| `packages/rhd_chat/src/stream.rs` | `packages/rhd_chat/src/stream/tests.rs` |
| `packages/rhd_chat/src/projects.rs` | `packages/rhd_chat/src/projects/tests.rs` |
| `packages/rhd_app/src/project_loader.rs` | `packages/rhd_app/src/project_loader/tests.rs` |
| `packages/rhd_ai/src/config.rs` | `packages/rhd_ai/src/config/tests.rs` |
| `packages/rhd_app/src/project_manager.rs` | `packages/rhd_app/src/project_manager/tests.rs` |
| `packages/rhd_app/src/template_loader.rs` | `packages/rhd_app/src/template_loader/tests.rs` |
| `packages/rhd_app/src/scenario/placeholder.rs` | `packages/rhd_app/src/scenario/placeholder/tests.rs` |
| `packages/rhd_db/src/lib.rs` | `packages/rhd_db/src/tests.rs` |
| `packages/rhd_util/src/lib.rs` | `packages/rhd_util/src/tests.rs` |

All files now use `#[cfg(test)] mod tests;` to reference the extracted test modules.

---

## Phase 2: Split Large Files Into Modules - PENDING

The following files exceed 400 lines and need to be split into logical submodules:

### 2.1 `packages/rhd_ai/src/client.rs` (879 lines)

**Proposed Split:**

```
packages/rhd_ai/src/client/
├── mod.rs              # Module declarations, OpenAiClient struct, re-exports
├── types.rs            # Request/Response types, ChatMessage, ToolCall
├── chat.rs             # Non-streaming chat methods
├── stream.rs           # Streaming implementation
└── sse.rs              # SSE parsing logic
```

**Rationale:**
- `types.rs`: Contains `ChatRequest`, `ChatMessage`, `ToolCall`, `FunctionCall`, `ChatResponse`, etc.
- `chat.rs`: Contains `chat`, `chat_with_tools`
- `stream.rs`: Contains `chat_stream`, `chat_stream_with_tools`
- `sse.rs`: Contains SSE event parsing, tool call parsing from stream

### 2.2 `packages/rhd_app/src/ws.rs` (873 lines)

**Proposed Split:**

```
packages/rhd_app/src/ws/
├── mod.rs              # Module declarations, server setup, re-exports
├── connection.rs       # Connection handling logic
├── handlers.rs         # All handle_* functions
└── events.rs           # Chat event to WebSocket event conversion
```

**Rationale:**
- `connection.rs`: Contains `handle_ws_connection` with the main event loop
- `handlers.rs`: Contains all `handle_run_scenario`, `handle_create_chat`, `handle_list_chats`, etc.
- `events.rs`: Contains the chat event to WebSocket event conversion logic

### 2.3 `packages/rhd_chat/src/tools/mod.rs` (753 lines)

**Proposed Split:**

```
packages/rhd_chat/src/tools/
├── mod.rs              # Module declarations and re-exports
├── builtin.rs          # Built-in tool definitions and handlers
├── loop.rs             # tool_loop function
├── messages.rs         # Message building utilities
└── utils.rs            # Utility functions (split_tool_name, etc.)
```

**Rationale:**
- `builtin.rs`: Contains `rhd_set_todo_list_tool_definition`, `rhd_set_role_tool_definition`, `collect_builtin_tools`, `handle_rhd_set_todo_list`, `handle_rhd_set_role`, `parse_todo_list`, `inject_todo_list_message`, `render_environment_details_for_injection`
- `loop.rs`: Contains `tool_loop`, `execute_tool_call`, `collect_tools_from_projects`
- `messages.rs`: Contains `build_chat_messages`, `build_chat_messages_for_tools`
- `utils.rs`: Contains `split_tool_name`, `extract_mcp_id_from_tool_name`

### 2.4 `packages/rhd_db/src/chat_db/mod.rs` (620 lines)

**Proposed Split:**

```
packages/rhd_db/src/chat_db/
├── mod.rs              # Module declarations, struct definitions, re-exports
├── schema.rs           # Database initialization and migration
├── chats.rs            # Chat CRUD operations
├── messages.rs         # Message operations
└── projects.rs         # Project-related operations
```

**Rationale:**
- `schema.rs`: Contains `init`, `migrate`, and other schema-related methods
- `chats.rs`: Contains `create_chat`, `list_chats`, `get_chat`, `delete_chat`, `delete_all_chats`
- `messages.rs`: Contains `add_message`, `get_messages`, `get_todo_list`, `set_todo_list`
- `projects.rs`: Contains `attach_project`, `detach_project`, `get_chat_projects`

### 2.5 `packages/rhd_app/src/daemon.rs` (615 lines)

**Proposed Split:**

```
packages/rhd_app/src/daemon/
├── mod.rs              # Module declarations, state structs, re-exports
├── run.rs              # run_daemon function
├── ipc.rs              # IPC handling
└── reload.rs           # Reload and shutdown logic
```

**Rationale:**
- `run.rs`: Contains `run_daemon`
- `ipc.rs`: Contains IPC message handling
- `reload.rs`: Contains reload configuration and shutdown logic

### 2.6 `packages/rhd_api/src/lib.rs` (593 lines)

**Proposed Split:**

```
packages/rhd_api/src/
├── lib.rs              # Module declarations and re-exports
├── execution.rs        # Execution tracking types
├── chat.rs             # Chat-related DTOs and events
├── ws.rs               # WebSocket protocol types
└── project.rs          # (already exists)
```

**Rationale:**
- `execution.rs`: Contains `ExecutionEvent`, `EventType`, `EventData`, `StepTiming`, `LogSection`, `ScenarioMeta`, `ScenarioStatus`
- `chat.rs`: Contains `ChatMessageDto`, `ChatInfo`, `TodoItemDto`, `TodoListUpdatedEvent`, etc.
- `ws.rs`: Contains `WsRequest`, `WsResponse`, `WsEvent`, `ErrorCode`

### 2.7 `packages/rhd_chat/src/stream.rs` (516 lines)

**Proposed Split:**

```
packages/rhd_chat/src/stream/
├── mod.rs              # Module declarations, TemplateLoaderRef, re-exports
├── contract.rs         # inject_todo_tool_contract
└── send.rs             # send_message function
```

**Rationale:**
- `contract.rs`: Contains `inject_todo_tool_contract`
- `send.rs`: Contains `send_message`

### 2.8 `packages/rhd_app/src/scenario/ai_chat.rs` (475 lines)

**Proposed Split:**

```
packages/rhd_app/src/scenario/ai_chat/
├── mod.rs              # Module declarations, execute_ai_chat, re-exports
├── simple.rs           # Non-MCP execution path
├── mcp.rs              # MCP execution path
└── utils.rs            # Helper functions (apply_model_aliases, etc.)
```

**Rationale:**
- `simple.rs`: Contains the non-MCP execution loop
- `mcp.rs`: Contains the MCP execution loop
- `utils.rs`: Contains `apply_model_aliases` and other helpers

### 2.9 `packages/rhd_app/src/execution.rs` (410 lines)

**Proposed Split:**

```
packages/rhd_app/src/execution/
├── mod.rs              # Module declarations, struct definitions, re-exports
├── tracker.rs          # ExecutionTracker methods
└── handle.rs           # ExecutionHandle methods
```

**Rationale:**
- `tracker.rs`: Contains `ExecutionTracker::new`, `start`, `finish`, `abort`, etc.
- `handle.rs`: Contains `ExecutionHandle` methods

---

## Phase 3: Extract Common Code to Utility Functions - PENDING

### 3.1 Identify Common Patterns

After splitting files, identify common patterns that can be extracted:

1. **Error handling patterns**: Common error handling logic
2. **JSON serialization/deserialization**: Common JSON utilities
3. **Date/time formatting**: Common timestamp utilities
4. **String manipulation**: Common string utilities

### 3.2 Move to `rhd_util`

Functions that are used across multiple crates should be moved to `rhd_util`:

1. **Environment variable substitution**: Already in `rhd_util`
2. **Common error types**: Consider moving shared error types
3. **Common traits**: Consider moving shared traits

---

## Phase 4: Move Cross-Crate Utilities to rhd_util - PENDING

Identify utilities that are duplicated or could be shared across crates and move them to `rhd_util`.

---

## Implementation Order

### Recommended Order:

1. **Phase 1**: Extract all test modules first (low risk, high value) - **COMPLETE**
   - Start with largest test modules
   - Verify tests still pass after each extraction

2. **Phase 2**: Split large files
   - Start with `tools.rs` (largest file)
   - Work through the list in order of size
   - Verify compilation and tests after each split

3. **Phase 3**: Extract common utilities
   - Identify patterns after splitting
   - Move shared code to `rhd_util`

4. **Phase 4**: Final cleanup
   - Update documentation
   - Verify all tests pass
   - Run clippy and fix warnings

---

## Risk Mitigation

1. **Incremental changes**: Make small, verifiable changes
2. **Test after each step**: Run `cargo test` after each file modification
3. **Use git branches**: Consider creating a feature branch for this refactoring
4. **Preserve public API**: Ensure public interfaces remain unchanged
5. **Update imports**: Carefully update all `use` statements

---

## Success Criteria

1. All inline `#[cfg(test)]` modules extracted to separate files - **COMPLETE**
2. No source file exceeds 400 lines (excluding test files)
3. All tests pass
4. No clippy warnings
5. Code compiles without errors
6. Public API remains unchanged

### 1.1 Large Test Modules (>200 lines)

These files have substantial test modules that should be extracted to `tests.rs` or `tests/mod.rs`:

#### `packages/rhd_chat/src/tools.rs` (~942 lines of tests)
- **Action**: Extract to `packages/rhd_chat/src/tools/tests.rs`
- **Reason**: Test module is larger than the code itself

#### `packages/rhd_db/src/chat_db.rs` (~507 lines of tests)
- **Action**: Extract to `packages/rhd_db/src/chat_db/tests.rs`
- **Reason**: Test module is substantial

#### `packages/rhd_api/src/lib.rs` (~389 lines of tests)
- **Action**: Extract to `packages/rhd_api/src/tests.rs`
- **Reason**: Test module is substantial

#### `packages/rhd_app/src/project_loader.rs` (~258 lines of tests)
- **Action**: Extract to `packages/rhd_app/src/project_loader/tests.rs`
- **Reason**: Test module is substantial

#### `packages/rhd_chat/src/projects.rs` (~237 lines of tests)
- **Action**: Extract to `packages/rhd_chat/src/projects/tests.rs`
- **Reason**: Test module is substantial

#### `packages/rhd_chat/src/stream.rs` (~182 lines of tests)
- **Action**: Extract to `packages/rhd_chat/src/stream/tests.rs`
- **Reason**: Test module is substantial

#### `packages/rhd_app/src/project_manager.rs` (~163 lines of tests)
- **Action**: Extract to `packages/rhd_app/src/project_manager/tests.rs`
- **Reason**: Test module is substantial

### 1.2 Medium Test Modules (50-200 lines)

#### `packages/rhd_app/src/ws.rs` (~66 lines of tests)
- **Action**: Extract to `packages/rhd_app/src/ws/tests.rs`
- **Reason**: Consistency, even though small

#### `packages/rhd_ai/src/config.rs` (~92 lines of tests)
- **Action**: Extract to `packages/rhd_ai/src/config/tests.rs`
- **Reason**: Consistency

### 1.3 Small Test Modules (<50 lines)

#### `packages/rhd_app/src/template_loader.rs` (~86 lines of tests)
- **Action**: Extract to `packages/rhd_app/src/template_loader/tests.rs`
- **Reason**: Consistency

#### `packages/rhd_app/src/scenario/placeholder.rs` (~52 lines of tests)
- **Action**: Extract to `packages/rhd_app/src/scenario/placeholder/tests.rs`
- **Reason**: Consistency

#### `packages/rhd_db/src/lib.rs` (~40 lines of tests)
- **Action**: Extract to `packages/rhd_db/src/tests.rs`
- **Reason**: Consistency

#### `packages/rhd_util/src/lib.rs` (~43 lines of tests)
- **Action**: Extract to `packages/rhd_util/src/tests.rs`
- **Reason**: Consistency

---

## Phase 2: Split Large Files Into Modules

### 2.1 `packages/rhd_chat/src/tools.rs` (~750 lines after test extraction)

**Current Structure Analysis:**
- Lines 1-121: Tool definitions and collection functions
- Lines 123-181: `handle_rhd_set_todo_list`
- Lines 183-216: `parse_todo_list`
- Lines 218-275: `inject_todo_list_message`
- Lines 277-321: `render_environment_details_for_injection`
- Lines 323-397: `handle_rhd_set_role`
- Lines 399-646: `tool_loop` (main tool execution loop)
- Lines 648-685: `execute_tool_call`
- Lines 687-749: Utility functions (`split_tool_name`, `build_chat_messages`, etc.)

**Proposed Split:**

```
packages/rhd_chat/src/tools/
├── mod.rs              # Module declarations and re-exports
├── builtin.rs          # Built-in tool definitions and handlers
├── loop.rs             # tool_loop function
├── messages.rs         # Message building utilities
└── utils.rs            # Utility functions (split_tool_name, etc.)
```

**Rationale:**
- `builtin.rs`: Contains `rhd_set_todo_list_tool_definition`, `rhd_set_role_tool_definition`, `collect_builtin_tools`, `handle_rhd_set_todo_list`, `handle_rhd_set_role`, `parse_todo_list`, `inject_todo_list_message`, `render_environment_details_for_injection`
- `loop.rs`: Contains `tool_loop`, `execute_tool_call`, `collect_tools_from_projects`
- `messages.rs`: Contains `build_chat_messages`, `build_chat_messages_for_tools`
- `utils.rs`: Contains `split_tool_name`, `extract_mcp_id_from_tool_name`

### 2.2 `packages/rhd_db/src/chat_db.rs` (~617 lines after test extraction)

**Current Structure Analysis:**
- Lines 1-38: Struct definitions (`ChatInfo`, `Message`, `ChatDb`)
- Lines 40-98: `ChatDb::new`, `init`
- Lines 100-200: Migration and schema methods
- Lines 200-400: Chat CRUD operations
- Lines 400-617: Message and project operations

**Proposed Split:**

```
packages/rhd_db/src/chat_db/
├── mod.rs              # Module declarations, struct definitions, re-exports
├── schema.rs           # Database initialization and migration
├── chats.rs            # Chat CRUD operations
├── messages.rs         # Message operations
└── projects.rs         # Project-related operations
```

**Rationale:**
- `schema.rs`: Contains `init`, `migrate`, and other schema-related methods
- `chats.rs`: Contains `create_chat`, `list_chats`, `get_chat`, `delete_chat`, `delete_all_chats`
- `messages.rs`: Contains `add_message`, `get_messages`, `get_todo_list`, `set_todo_list`
- `projects.rs`: Contains `attach_project`, `detach_project`, `get_chat_projects`

### 2.3 `packages/rhd_api/src/lib.rs` (~590 lines after test extraction)

**Current Structure Analysis:**
- Lines 1-100: Execution tracking types (`ExecutionEvent`, `EventType`, `EventData`)
- Lines 100-200: Step timing and logging types
- Lines 200-400: Chat-related DTOs and events
- Lines 400-590: WebSocket protocol types

**Proposed Split:**

```
packages/rhd_api/src/
├── lib.rs              # Module declarations and re-exports
├── execution.rs        # Execution tracking types
├── chat.rs             # Chat-related DTOs and events
├── ws.rs               # WebSocket protocol types
└── project.rs          # (already exists)
```

**Rationale:**
- `execution.rs`: Contains `ExecutionEvent`, `EventType`, `EventData`, `StepTiming`, `LogSection`, `ScenarioMeta`, `ScenarioStatus`
- `chat.rs`: Contains `ChatMessageDto`, `ChatInfo`, `TodoItemDto`, `TodoListUpdatedEvent`, etc.
- `ws.rs`: Contains `WsRequest`, `WsResponse`, `WsEvent`, `ErrorCode`

### 2.4 `packages/rhd_app/src/ws.rs` (~847 lines after test extraction)

**Current Structure Analysis:**
- Lines 1-44: WebSocket server setup (`run_ws_server`)
- Lines 46-274: Connection handling (`handle_ws_connection`)
- Lines 276-847: Message handlers (various `handle_*` functions)

**Proposed Split:**

```
packages/rhd_app/src/ws/
├── mod.rs              # Module declarations, server setup, re-exports
├── connection.rs       # Connection handling logic
├── handlers.rs         # All handle_* functions
└── events.rs           # Chat event to WebSocket event conversion
```

**Rationale:**
- `connection.rs`: Contains `handle_ws_connection` with the main event loop
- `handlers.rs`: Contains all `handle_run_scenario`, `handle_create_chat`, `handle_list_chats`, etc.
- `events.rs`: Contains the chat event to WebSocket event conversion logic

### 2.5 `packages/rhd_ai/src/client.rs` (879 lines, no tests)

**Current Structure Analysis:**
- Lines 1-47: Error types (`AiError`)
- Lines 49-150: Request/Response types (`ChatRequest`, `ChatMessage`, `ToolCall`)
- Lines 150-400: `OpenAiClient` implementation (chat methods)
- Lines 400-700: Streaming implementation
- Lines 700-879: SSE parsing and tool call parsing

**Proposed Split:**

```
packages/rhd_ai/src/client/
├── mod.rs              # Module declarations, OpenAiClient struct, re-exports
├── types.rs            # Request/Response types, ChatMessage, ToolCall
├── chat.rs             # Non-streaming chat methods
├── stream.rs           # Streaming implementation
└── sse.rs              # SSE parsing logic
```

**Rationale:**
- `types.rs`: Contains `ChatRequest`, `ChatMessage`, `ToolCall`, `FunctionCall`, `ChatResponse`, etc.
- `chat.rs`: Contains `chat`, `chat_with_tools`
- `stream.rs`: Contains `chat_stream`, `chat_stream_with_tools`
- `sse.rs`: Contains SSE event parsing, tool call parsing from stream

### 2.6 `packages/rhd_chat/src/stream.rs` (~513 lines after test extraction)

**Current Structure Analysis:**
- Lines 1-34: `TemplateLoaderRef` struct
- Lines 36-78: `inject_todo_tool_contract`
- Lines 80-513: `send_message` (main streaming function)

**Proposed Split:**

This file is borderline. After test extraction, it will be ~513 lines. Consider splitting only if `send_message` can be logically decomposed.

**Proposed Split (if needed):**

```
packages/rhd_chat/src/stream/
├── mod.rs              # Module declarations, TemplateLoaderRef, re-exports
├── contract.rs         # inject_todo_tool_contract
└── send.rs             # send_message function
```

**Rationale:**
- `contract.rs`: Contains `inject_todo_tool_contract`
- `send.rs`: Contains `send_message`

### 2.7 `packages/rhd_app/src/daemon.rs` (615 lines, no tests)

**Current Structure Analysis:**
- Lines 1-58: State structs (`ResolvedConfigPaths`, `ReloadableInner`, `DaemonState`)
- Lines 60-200: `run_daemon` function
- Lines 200-400: IPC handling
- Lines 400-615: Reload and shutdown logic

**Proposed Split:**

```
packages/rhd_app/src/daemon/
├── mod.rs              # Module declarations, state structs, re-exports
├── run.rs              # run_daemon function
├── ipc.rs              # IPC handling
└── reload.rs           # Reload and shutdown logic
```

**Rationale:**
- `run.rs`: Contains `run_daemon`
- `ipc.rs`: Contains IPC message handling
- `reload.rs`: Contains reload configuration and shutdown logic

### 2.8 `packages/rhd_app/src/scenario/ai_chat.rs` (475 lines, no tests)

**Current Structure Analysis:**
- Lines 1-100: `execute_ai_chat` function setup
- Lines 100-300: Main execution loop (non-MCP path)
- Lines 300-475: MCP execution path and helper functions

**Proposed Split:**

```
packages/rhd_app/src/scenario/ai_chat/
├── mod.rs              # Module declarations, execute_ai_chat, re-exports
├── simple.rs           # Non-MCP execution path
├── mcp.rs              # MCP execution path
└── utils.rs            # Helper functions (apply_model_aliases, etc.)
```

**Rationale:**
- `simple.rs`: Contains the non-MCP execution loop
- `mcp.rs`: Contains the MCP execution loop
- `utils.rs`: Contains `apply_model_aliases` and other helpers

### 2.9 `packages/rhd_app/src/execution.rs` (410 lines, no tests)

**Current Structure Analysis:**
- Lines 1-81: Struct definitions (`PauseNotification`, `ExecutionTracker`, `ActiveExecution`, `AbortHandle`)
- Lines 83-200: `ExecutionTracker` methods
- Lines 200-410: `ExecutionHandle` methods

**Proposed Split:**

```
packages/rhd_app/src/execution/
├── mod.rs              # Module declarations, struct definitions, re-exports
├── tracker.rs          # ExecutionTracker methods
└── handle.rs           # ExecutionHandle methods
```

**Rationale:**
- `tracker.rs`: Contains `ExecutionTracker::new`, `start`, `finish`, `abort`, etc.
- `handle.rs`: Contains `ExecutionHandle` methods

---

## Phase 3: Extract Common Code to Utility Functions

### 3.1 Identify Common Patterns

After splitting files, identify common patterns that can be extracted:

1. **Error handling patterns**: Common error handling logic
2. **JSON serialization/deserialization**: Common JSON utilities
3. **Date/time formatting**: Common timestamp utilities
4. **String manipulation**: Common string utilities

### 3.2 Move to `rhd_util`

Functions that are used across multiple crates should be moved to `rhd_util`:

1. **Environment variable substitution**: Already in `rhd_util`
2. **Common error types**: Consider moving shared error types
3. **Common traits**: Consider moving shared traits

---

## Phase 4: Implementation Order

### Recommended Order:

1. **Phase 1**: Extract all test modules first (low risk, high value)
   - Start with largest test modules
   - Verify tests still pass after each extraction

2. **Phase 2**: Split large files
   - Start with `tools.rs` (largest file)
   - Work through the list in order of size
   - Verify compilation and tests after each split

3. **Phase 3**: Extract common utilities
   - Identify patterns after splitting
   - Move shared code to `rhd_util`

4. **Phase 4**: Final cleanup
   - Update documentation
   - Verify all tests pass
   - Run clippy and fix warnings

---

## Risk Mitigation

1. **Incremental changes**: Make small, verifiable changes
2. **Test after each step**: Run `cargo test` after each file modification
3. **Use git branches**: Consider creating a feature branch for this refactoring
4. **Preserve public API**: Ensure public interfaces remain unchanged
5. **Update imports**: Carefully update all `use` statements

---

## Success Criteria

1. All inline `#[cfg(test)]` modules extracted to separate files
2. No source file exceeds 400 lines (excluding test files)
3. All tests pass
4. No clippy warnings
5. Code compiles without errors
6. Public API remains unchanged
