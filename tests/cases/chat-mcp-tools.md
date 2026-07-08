# Test Case: Chat with MCP Tools

## Description
System handles chat with MCP tools: user attaches project with MCP, sends message requesting tool usage, AI calls tool, receives result, streams final response to user.

## Preconditions
- Chat exists and is selected
- Project with MCP configuration available
- MCP server can be started successfully

## Steps
1. User creates new chat
2. User attaches project with MCP configuration
3. System spawns MCP server, emits `ProjectAttached` event
4. User sends message asking to use specific MCP tool
5. System collects tools from attached projects
6. System calls AI with tool definitions (non-streaming)
7. AI returns tool call request
8. System emits `ToolCallStarted` event
9. System executes MCP tool
10. System emits `ToolCallCompleted` event
11. System feeds tool result back to AI
12. AI returns final response (no more tool calls)
13. System streams final response to user via `StreamChunk` events
14. System emits `StreamFinished` event
15. System persists assistant message to database

## Expected Results
- `ProjectAttached` event received after attaching project
- `ToolCallStarted` event received with tool name and arguments
- `ToolCallCompleted` event received with tool result
- Multiple `StreamChunk` events received for final response
- `StreamFinished` event received with message ID
- Final assistant message displayed in chat with correct content
- Message has real numeric ID (not optimistic temp ID)

## Events Flow
```
createChat → MessageAdded (user)
attachProject → ProjectAttached
sendMessage → MessageAdded (user)
  → [internal: chat_with_tools non-streaming]
  → ToolCallStarted
  → ToolCallCompleted
  → [internal: chat_stream_cancellable streaming]
  → StreamChunk (multiple)
  → MessageAdded (assistant)
  → StreamFinished
```

## Actions
- `createChat` — create new chat
- `attachProject` — attach project with MCP
- `sendMessage` — send user message
- `ProjectAttached` — system event after project attached
- `ToolCallStarted` — system event when tool call begins
- `ToolCallCompleted` — system event when tool call finishes
- `StreamChunk` — system event with streaming content chunk
- `StreamFinished` — system event when streaming complete
- `MessageAdded` — system event when message persisted

## Covered By
- `frontend/src/tests/e2e/chat-mcp-tools.test.ts` (E2E test)
- `packages/rhd_chat/src/tools.rs` (Rust unit tests)
