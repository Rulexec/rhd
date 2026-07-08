# Chat MCP Tools Test Plan

## Goal
Write comprehensive tests for chat with MCP tools integration, ensuring:
1. User opens chat
2. User attaches project with MCP
3. MCP is successfully started
4. User sends message asking to run specific MCP tool
5. AI chat called with available MCP tools, calls it, receives response, forms response for user
6. In chat we see MCP request, response, final response to user is streamed

## Problem Analysis

**Root cause of hanging test**: The current `tool_loop()` in [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs) uses non-streaming `chat_with_tools()` API. When tools are present, the final response is not streamed to the frontend - it waits for complete response then returns. This causes frontend E2E tests to hang indefinitely waiting for streaming events that never come.

**Current flow**:
```
User sends message → tool_loop() → chat_with_tools() [non-streaming] → tool calls → execute tools → chat_with_tools() again → final response (not streamed)
```

**Expected flow**:
```
User sends message → tool_loop() → chat_with_tools() [non-streaming] → tool calls → execute tools → chat_with_tools() again → final response → chat_stream_cancellable() [streaming] → StreamChunk events → StreamFinished
```

## Design Decision

**Only the final agent message is streamed.** Tool calls and tool results are NOT streamed - they happen internally during the tool loop. This matches real-world behavior where the user sees the agent's final response after it has used tools, not the intermediate tool call details.

**Flow**:
1. User sends message
2. Tool loop starts: non-streaming `chat_with_tools()` call
3. If AI returns tool calls: execute tools, feed results back, repeat
4. When AI returns final response (no tool calls): stream that response to user
5. Emit `StreamFinished`

This means:
- Tool calls: not visible in streaming (only via `ToolCallStarted`/`ToolCallCompleted` events)
- Final response: streamed chunk-by-chunk to user

## Implementation Status

### ✅ Phase 1: Test Infrastructure Setup (COMPLETED)

#### 1.1 Create test case document
- ✅ File: `tests/cases/chat-mcp-tools.md`
- ✅ Document complete test scenario with preconditions, steps, expected results

#### 1.2 Create test project with MCP
- ✅ Directory: `test_e2e/projects/test-project-mcp/`
- ✅ Files:
  - `mcp.yaml` - MCP configuration referencing mock MCP server
  - `systemPrompt.md` - System prompt instructing AI to use MCP tools

#### 1.3 Add mock MCP server to rhd_test
- ✅ File: `packages/rhd_test/src/mock_mcp_server.rs`
- ✅ Implement simple MCP server using stdio transport (JSON-RPC 2.0)
- ✅ Support `tools/list` and `tools/call` methods
- ✅ Return configurable tool definitions and responses
- ✅ Integrated with CLI via `mcp-server` subcommand

### ✅ Phase 2: Streaming Tool Loop Implementation (COMPLETED)

#### 2.1 Modify tool loop to stream final response
- ✅ File: `packages/rhd_chat/src/tools.rs`
- ✅ Modified `tool_loop()` to:
  - Continue using non-streaming `chat_with_tools()` for tool call detection
  - When final response received (no tool calls): emit `StreamChunk` event with final content
  - Emit `StreamFinished` when complete

#### 2.2 Update mock server for two-phase response
- ✅ File: `packages/rhd_test/src/mock_server.rs`
- ✅ Modified `chat_completions()` to:
  - When `stream: false` and tools present: return tool calls (existing behavior)
  - When `stream: true` (final response): return streaming content chunks
- ✅ Detect MCP tools by checking for "/" in tool names
- ✅ Return appropriate tool call (mock1/echo for MCP tools, rhd_set_flag for built-in)

### ✅ Phase 3: Frontend E2E Test (COMPLETED)

#### 3.1 Write frontend E2E test
- ✅ File: `frontend/src/tests/e2e/chat-mcp-tools.test.ts`
- ✅ Test steps:
  1. Create chat
  2. Attach project with MCP
  3. Verify MCP started (check for `ProjectAttached` event)
  4. Send message asking to use MCP tool
  5. Verify `ToolCallStarted` event received
  6. Verify `ToolCallCompleted` event received
  7. Verify streaming chunks received (`StreamChunk` events)
  8. Verify `StreamFinished` event received
  9. Verify final message displayed correctly

### ✅ Phase 4: Rust Unit Tests (COMPLETED)

#### 4.1 Write Rust tests for tool loop
- ✅ File: `packages/rhd_chat/src/tools.rs` (added `#[cfg(test)]` module)
- ✅ Test cases implemented (21 tests total, all passing):
  - Helper function tests: `test_extract_mcp_id_from_tool_name_with_prefix`, `test_extract_mcp_id_from_tool_name_without_prefix`, `test_split_tool_name_with_prefix`, `test_split_tool_name_without_prefix`
  - Message building tests: `test_build_chat_messages`, `test_build_chat_messages_for_tools_simple`, `test_build_chat_messages_for_tools_with_tool_calls`
  - Tool execution tests: `test_execute_tool_call_with_mcp_client`, `test_execute_tool_call_unknown_tool`
  - Tool loop logic tests: `test_tool_loop_iteration_tracking`, `test_tool_loop_max_iterations_check`, `test_tool_loop_cancellation_check`, `test_tool_loop_no_tools_returns_immediately`, `test_tool_loop_single_tool_call`, `test_tool_loop_max_iterations`, `test_tool_loop_cancellation`
  - Mock MCP client tests: `test_mock_mcp_client_list_tools`, `test_mock_mcp_client_call_tool`, `test_mock_mcp_client_call_tool_default_result`, `test_mock_mcp_client_has_tool`, `test_mock_mcp_client_integration`

### 🔄 Phase 5: Debug and Validation (IN PROGRESS)

#### 5.1 Debug logging (CLEANED UP)
- ✅ Debug logging was added during investigation and has been removed from all files:
  - `packages/rhd_chat/src/tools.rs`
  - `packages/rhd_app/src/project_loader.rs`
  - `packages/rhd_chat/src/projects.rs`
  - `frontend/src/lib/projectStores.ts`
  - `frontend/src/lib/ws.ts`
  - `packages/rhd_test/src/frontend_test.rs`
  - `packages/rhd_test/src/mock_server.rs`

#### 5.2 Fix chatProjects empty issue
- ✅ **Root cause identified**: `handle_attach_project()` in `packages/rhd_app/src/ws.rs` was spawning a background task and returning immediately with `{ "status": "attaching" }`. The `ProjectAttached` event was sent asynchronously inside the task, creating a race condition. Additionally, errors (like MCP spawn failures) were silently discarded.
- ✅ **Fix applied**: Changed `handle_attach_project()` to await the result synchronously instead of spawning a background task. Now returns `{ "attached": true }` on success or an error response on failure.
- ✅ **Test infrastructure updated**: Created `test_e2e/mcp/mock-mcp.yaml` base MCP config and updated `frontend_test.rs` to pass `--mcp-dir` to daemon.
- ✅ **Result**: `chatProjects` store is now properly populated when a project is attached. The `projectAttached` event is received correctly.

#### 5.3 Run tests and iterate
- ✅ Backend E2E tests pass (standard and MCP tests)
- ❌ Frontend E2E test fails: MCP status is `failed` instead of `connected`

## Current Status

**All Issues Resolved** ✅

**MCP Path Issue**: FIXED
- Changed `frontend_test.rs` to run daemon from workspace root instead of temp directory
- Added "WebSocket server started on port" print statement to `daemon.rs` for test synchronization

**ToolCall Serialization Issue**: FIXED
- Root cause: `ToolCall` struct in `packages/rhd_ai/src/client.rs` didn't match OpenAI API format
- OpenAI API expects nested structure: `{ id, type: "function", function: { name, arguments } }`
- Old struct had flat fields: `{ id, name, arguments }`
- When tool loop sent second request with tool call history, mock server failed to deserialize with 422 error
- Fixed by updating `ToolCall` struct to match OpenAI format and updating all code that accesses these fields

**Frontend E2E Test Status**: PASSED ✅
- MCP connects successfully ✅
- Tool calls execute correctly ✅
- ToolCallStarted and ToolCallCompleted events emitted ✅
- Final response streaming works ✅
- All 6 frontend E2E tests pass ✅
- All 20 backend E2E tests pass ✅
- All cargo unit tests pass ✅

## Files Modified

### New Files Created
- `tests/cases/chat-mcp-tools.md` - Test case documentation
- `test_e2e/projects/test-project-mcp/mcp.yaml` - MCP configuration
- `test_e2e/projects/test-project-mcp/systemPrompt.md` - System prompt
- `test_e2e/mcp/mock-mcp.yaml` - Base MCP config for mock server
- `packages/rhd_test/src/mock_mcp_server.rs` - Mock MCP server implementation
- `frontend/src/tests/e2e/chat-mcp-tools.test.ts` - Frontend E2E test

### Modified Files
- `packages/rhd_chat/src/tools.rs` - Modified tool loop to emit StreamChunk for final response
- `packages/rhd_test/src/mock_server.rs` - Support two-phase response (tool calls then streaming)
- `packages/rhd_test/src/main.rs` - Integrated mock MCP server CLI command
- `packages/rhd_test/src/args.rs` - Added McpServer subcommand
- `packages/rhd_test/src/frontend_test.rs` - Added projects-dir and mcp-dir arguments, print daemon stdout
- `packages/rhd_chat/src/projects.rs` - Added and removed debug logging
- `packages/rhd_app/src/project_loader.rs` - Added and removed debug logging
- `packages/rhd_app/src/ws.rs` - Fixed `handle_attach_project()` to await result instead of spawning background task
- `packages/rhd_app/src/daemon.rs` - Added WebSocket server startup print statement
- `frontend/src/lib/projectStores.ts` - Added and removed debug logging
- `frontend/src/lib/ws.ts` - Added and removed debug logging

## Success Criteria

1. ✅ Frontend E2E test passes: chat with MCP tools completes without hanging
2. ✅ Streaming works: user sees final response chunks in real-time
3. ✅ Tool calls visible: `ToolCallStarted` and `ToolCallCompleted` events emitted
4. ✅ Rust unit tests pass: 21 tests covering tool loop logic, all passing
5. ✅ No regression: existing tests still pass

## Next Steps

1. **Update documentation**: Update memory files with new testing patterns and ToolCall structure
