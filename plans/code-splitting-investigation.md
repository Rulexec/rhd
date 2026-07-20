# Code Splitting Investigation Plan

## Overview

This plan details the splitting of 9 files that exceed the 400-line limit into smaller, logical modules.

## Files to Split

### 1. `packages/rhd_chat/src/tools/tests.rs` (945 lines) 🔴 Critical ✅ COMPLETED

**Original Structure:**
- Helper functions and `MockProjectProvider` (lines 1-56)
- Tool name parsing tests (lines 60-85)
- Message building tests (lines 87-150)
- MockMcpClient tests (lines 414-472)
- Template loader and builtin tools tests (lines 474-600)
- Todo list parsing and handling tests (lines 700-830)
- Todo list injection tests (lines 790-945)

**Actual Split:**
```
packages/rhd_chat/src/tools/tests/
├── mod.rs              # Module declarations (7 lines)
├── helpers.rs          # MockProjectProvider, cleanup (49 lines)
├── tool_parsing.rs     # Tool name parsing tests (27 lines)
├── mock_mcp.rs         # MockMcpClient tests (110 lines)
├── message_tests.rs    # Message building and tool loop tests (237 lines)
├── role_tests.rs       # Role-related tests and template loader (196 lines)
└── todo_list.rs        # Todo list related tests (279 lines)
```

**Result:** All files under 400 lines. All 46 tests pass.

---

### 2. `packages/rhd_app/src/ws/handlers.rs` (604 lines) 🔴 Critical ✅ COMPLETED

**Original Structure:**
- Main dispatch function `handle_ws_message` (lines 10-74)
- Scenario handlers (lines 88-226)
- Chat handlers (lines 228-349)
- Project handlers (lines 413-506)
- Role handlers (lines 518-604)

**Actual Split:**
```
packages/rhd_app/src/ws/handlers/
├── mod.rs          # Main dispatch function (78 lines)
├── scenario.rs     # Scenario-related handlers (170 lines)
├── chat.rs         # Chat-related handlers (184 lines)
├── project.rs      # Project-related handlers (100 lines)
└── role.rs         # Role-related handlers (93 lines)
```

**Result:** All files under 400 lines. All tests pass.

---

### 3. `packages/rhd_db/src/chat_db/tests.rs` (503 lines) 🟡 High ✅ COMPLETED

**Original Structure:**
- Chat CRUD tests (lines 10-94)
- Message tests (lines 97-150)
- Migration tests (lines 150-243)
- Project tests (lines 245-336)
- Role tests (lines 338-441)
- Todo list tests (lines 443-503)

**Actual Split:**
```
packages/rhd_db/src/chat_db/tests/
├── mod.rs              # Module declarations (7 lines)
├── helpers.rs          # cleanup helper (7 lines)
├── chat_tests.rs       # Chat CRUD tests (88 lines)
├── message_tests.rs    # Message tests (83 lines)
├── migration_tests.rs  # Migration tests (70 lines)
├── project_tests.rs    # Project tests (95 lines)
├── role_tests.rs       # Role tests (107 lines)
└── todo_tests.rs       # Todo list tests (64 lines)
```

**Result:** All files under 400 lines. All 23 tests pass.

---

### 4. `packages/rhd_ai/src/client/stream.rs` (490 lines) 🟡 High ✅ COMPLETED

**Original Structure:**
- `StreamExecutor` struct (lines 8-12)
- `chat_stream` method (lines 14-150)
- `chat_stream_cancellable` method (lines 150-273)
- `chat_stream_with_tools` method (lines 275-464)
- `ToolCallAccumulator` (lines 467-490)

**Actual Split:**
```
packages/rhd_ai/src/client/stream/
├── mod.rs      # Module declarations and StreamExecutor struct (11 lines)
├── simple.rs   # chat_stream and chat_stream_cancellable (268 lines)
└── tools.rs    # chat_stream_with_tools and ToolCallAccumulator (224 lines)
```

**Result:** All files under 400 lines. All 9 tests pass.

---

### 5. `packages/rhd_chat/src/stream/send.rs` (453 lines) 🟡 High ✅ COMPLETED

**Original Structure:**
- `send_message` function (lines 20-150)
- `send_message_with_tools` function (lines 150-220)
- `edit_and_resend` function (lines 222-348)
- `format_messages_for_log` function (lines 350-372)
- `handle_stream_result` function (lines 374-453)

**Actual Split:**
```
packages/rhd_chat/src/stream/send/
├── mod.rs      # Module declarations (6 lines)
├── message.rs  # send_message and edit_and_resend (303 lines)
├── tools.rs    # send_message_with_tools (61 lines)
└── utils.rs    # format_messages_for_log and handle_stream_result (116 lines)
```

**Result:** All files under 400 lines. All 46 tests pass.

---

### 6. `packages/rhd_test/src/standard_test.rs` (444 lines) 🟢 Medium ✅ COMPLETED

**Original Structure:**
- Single large `run_single_test` function (lines 10-444)

**Actual Split:**
```
packages/rhd_test/src/standard_test/
├── mod.rs          # Module declarations and run_single_test entry (61 lines)
├── setup.rs        # Setup and daemon spawning (92 lines)
├── execution.rs    # Test execution and scenario running (73 lines)
└── validation.rs   # Log and meta validation (312 lines)
```

**Result:** All files under 400 lines. Build succeeds.

---

### 7. `packages/rhd_test/src/mock_server.rs` (415 lines) 🟢 Medium ✅ COMPLETED

**Original Structure:**
- Type definitions (lines 19-100)
- `chat_completions` handler (lines 100-381)
- `start_mock_server` function (lines 383-415)

**Actual Split:**
```
packages/rhd_test/src/mock_server/
├── mod.rs          # Module declarations and start_mock_server (44 lines)
├── types.rs        # Type definitions (117 lines)
└── handlers.rs     # chat_completions handler (265 lines)
```

**Result:** All files under 400 lines. Build succeeds.

---

### 8. `frontend/src/lib/chatWs.ts` (712 lines) 🔴 Critical ✅ COMPLETED

**Original Structure:**
- Message parsing functions (lines 31-120)
- Chat operations (lines 120-345)
- `handleChatEvent` function (lines 347-696)
- Test exports (lines 698-712)

**Actual Split:**
```
frontend/src/lib/chatWs/
├── index.ts        # Main exports and re-exports (37 lines)
├── parsers.ts      # parseAssistantMessage, parseToolResult, mergeToolResults (117 lines)
├── operations.ts   # Chat operations (loadChats, sendMessage, etc.) (230 lines)
└── events.ts       # handleChatEvent function (349 lines)
```

**Result:** All files under 400 lines. Public API maintained through index.ts re-exports.

---

### 9. `frontend/src/components/Message.svelte` (499 lines) 🟡 High ✅ COMPLETED

**Original Structure:**
- Script section (lines 1-83)
- HTML template (lines 85-200)
- CSS styles (lines 200-499)

**Actual Split:**
```
frontend/src/components/
├── Message.svelte          # Main component with script and template (165 lines)
└── Message.styles.css      # Extracted CSS styles (335 lines)
```

**Result:** All files under 400 lines. CSS extracted to separate file, imported in component.

---

## Implementation Order

1. **Start with test files** (easier to split, less risk of breaking functionality):
   - `packages/rhd_chat/src/tools/tests.rs`
   - `packages/rhd_db/src/chat_db/tests.rs`

2. **Backend source files** (medium complexity):
   - `packages/rhd_ai/src/client/stream.rs`
   - `packages/rhd_chat/src/stream/send.rs`
   - `packages/rhd_test/src/mock_server.rs`
   - `packages/rhd_test/src/standard_test.rs`
   - `packages/rhd_app/src/ws/handlers.rs`

3. **Frontend files** (higher complexity due to UI concerns):
   - `frontend/src/lib/chatWs.ts`
   - `frontend/src/components/Message.svelte`

## Verification Steps

After each split:
1. Run `cargo check` to ensure no compilation errors
2. Run `cargo test --workspace` to verify functionality
3. Run `mise run check-large-files` to verify file size reduction

## Success Criteria

- All files under 400 lines
- All tests pass
- No compilation errors
- No unused import warnings
- Public API remains unchanged
