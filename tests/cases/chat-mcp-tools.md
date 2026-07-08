# Test Case: Chat with MCP Tools

## Description
System handles chat with MCP tools: user attaches project with MCP, sends message requesting tool usage, AI calls tool, receives result, streams final response to user. Tool calls are visible in the message list during streaming, and thinking content is preserved with intermediate assistant messages.

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
6. System calls AI with tool definitions (streaming)
7. AI streams thinking content via `ThinkingChunk` events
8. AI returns tool call request
9. System creates assistant message with thinking content and tool calls
10. System emits `ToolCallStarted` event
11. System executes MCP tool
12. System emits `ToolCallCompleted` event
13. System persists intermediate assistant message (with thinking and tool calls) to database
14. System persists tool result message to database
15. System feeds tool result back to AI
16. AI returns final response (no more tool calls)
17. System streams final response to user via `StreamChunk` events
18. System emits `StreamFinished` event
19. System persists final assistant message to database

## Expected Results
- `ProjectAttached` event received after attaching project
- `ThinkingChunk` events received during AI reasoning
- Assistant message created when tool call starts (visible in UI)
- `ToolCallStarted` event received with tool name and arguments
- `ToolCallCompleted` event received with tool result
- Multiple `StreamChunk` events received for final response
- `StreamFinished` event received with message ID
- Final assistant message displayed in chat with correct content
- Message has real numeric ID (not optimistic temp ID)
- After page reload, message order preserved: user → thinking + tool calls → tool result → final response

## Events Flow
```
createChat → MessageAdded (user)
attachProject → ProjectAttached
sendMessage → MessageAdded (user)
  → [internal: chat_stream_with_tools streaming]
  → ThinkingChunk (multiple, AI reasoning)
  → ToolCallStarted
  → ToolCallCompleted
  → MessageAdded (assistant with thinking + toolCalls)
  → MessageAdded (tool result)
  → [internal: next iteration or final response]
  → StreamChunk (multiple, final response)
  → MessageAdded (assistant final)
  → StreamFinished
```

## Actions
- `createChat` — create new chat
- `attachProject` — attach project with MCP
- `sendMessage` — send user message
- `ProjectAttached` — system event after project attached
- `ThinkingChunk` — system event with thinking/reasoning content chunk
- `ToolCallStarted` — system event when tool call begins
- `ToolCallCompleted` — system event when tool call finishes
- `StreamChunk` — system event with streaming content chunk
- `StreamFinished` — system event when streaming complete
- `MessageAdded` — system event when message persisted

## Covered By
- `frontend/src/tests/e2e/chat-mcp-tools.test.ts` (E2E test)
- `packages/rhd_chat/src/tools.rs` (Rust unit tests)
