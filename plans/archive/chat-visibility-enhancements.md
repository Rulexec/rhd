# Chat Visibility Enhancements Plan

## Overview
Add visibility for system prompts, AI thinking/reasoning content, and MCP tool calls in the chat UI. All three types of entries should be collapsed by default.

## Status: COMPLETED

---

## Phase 1: System Prompts Visibility ✅ COMPLETED

### Backend Changes
- [x] `packages/rhd_app/src/chat.rs`: Emit `MessageAdded` event after adding system prompt in `send_message` (line ~187)
- [x] `packages/rhd_app/src/chat.rs`: Emit `MessageAdded` event after adding system prompt in `edit_and_resend` (line ~666)

### Frontend Changes
- [x] `frontend/src/components/Message.svelte`: Added collapsible "System Prompt" section for `role="system"` messages (collapsed by default)

---

## Phase 2: AI Thinking/Reasoning Support ✅ COMPLETED

### Backend Changes
- [x] `packages/rhd_ai/src/client.rs`: Added `reasoning_content: Option<String>` to `StreamDelta` struct
- [x] `packages/rhd_ai/src/client.rs`: Added `reasoning_content: Option<String>` to `StreamChunk` struct
- [x] `packages/rhd_ai/src/client.rs`: Updated streaming parsing to extract `reasoning_content` from SSE delta
- [x] `packages/rhd_db/src/chat_db.rs`: Added `thinking_content TEXT` column migration
- [x] `packages/rhd_db/src/chat_db.rs`: Added `thinking_content: Option<String>` to `Message` struct
- [x] `packages/rhd_db/src/chat_db.rs`: Updated `add_message` to accept `thinking_content` parameter
- [x] `packages/rhd_db/src/chat_db.rs`: Updated `get_messages` and `get_message` to read `thinking_content`
- [x] `packages/rhd_api/src/lib.rs`: Added `ChatThinkingChunkEvent` struct
- [x] `packages/rhd_api/src/lib.rs`: Added `thinking_content: Option<String>` to `ChatMessageDto`
- [x] `packages/rhd_app/src/chat.rs`: Added `ThinkingChunk` variant to `ChatEvent` enum
- [x] `packages/rhd_app/src/chat.rs`: Updated streaming handlers to emit thinking chunks and accumulate thinking content
- [x] `packages/rhd_app/src/chat.rs`: Updated `add_message` calls to pass thinking_content
- [x] `packages/rhd_app/src/ws.rs`: Handle `ChatEvent::ThinkingChunk` and emit as `chatThinkingChunk` WebSocket event

### Frontend Changes
- [x] `frontend/src/lib/types/index.ts`: Added `thinkingContent` to `ChatMessageSchema`
- [x] `frontend/src/lib/types/ws.ts`: Added `ChatThinkingChunkEventSchema`
- [x] `frontend/src/lib/chatStores.ts`: Added `streamingThinkingContent` writable store
- [x] `frontend/src/lib/chatWs.ts`: Handle `chatThinkingChunk` event - accumulate to streaming message
- [x] `frontend/src/components/Message.svelte`: Added collapsible "Thinking" section (collapsed by default)
- [x] `frontend/src/components/StreamingMessage.svelte`: Added collapsible thinking content display while streaming
- [x] `frontend/src/components/MessageList.svelte`: Pass `thinkingContent` to `StreamingMessage`

---

## Phase 3: MCP Tool Calls Visualization ✅ COMPLETED

### Backend Changes ✅ COMPLETED
- [x] `packages/rhd_app/src/chat.rs`: Added `mcp_name: String` field to `ToolCallStarted` event variant
- [x] `packages/rhd_app/src/project_manager.rs`: Updated `get_mcp_clients` to return `Vec<(String, Arc<McpClient>)>` with MCP name
- [x] `packages/rhd_app/src/chat.rs`: Updated `collect_tools_from_projects` to return `Vec<(String, String, Arc<McpClient>)>` with (project_name, mcp_name, client)
- [x] `packages/rhd_app/src/chat.rs`: Updated `send_message_with_tools` signature
- [x] `packages/rhd_app/src/chat.rs`: Updated `tool_loop` signature
- [x] `packages/rhd_app/src/chat.rs`: Updated `execute_tool_call` to return `(String, String)` tuple (result, mcp_name)
- [x] `packages/rhd_app/src/chat.rs`: Added `find_mcp_for_tool` helper method
- [x] `packages/rhd_app/src/chat.rs`: Updated tool loop to find MCP name and pass it to `ToolCallStarted` event
- [x] `packages/rhd_api/src/lib.rs`: Added `mcp_name: String` field to `ToolCallStartedEvent`
- [x] `packages/rhd_app/src/ws.rs`: Updated to pass `mcp_name` when creating `ToolCallStartedEvent` payload

### Frontend Changes ✅ COMPLETED
  - [x] `frontend/src/lib/types/ws.ts`: Update `ToolCallStartedEventSchema` with `mcpName`
  - [x] `frontend/src/lib/types/index.ts`: Update `ToolCallSchema` with `mcpName`
  - [x] `frontend/src/lib/chatWs.ts`: Pass `mcpName` when creating tool call object in `toolCallStarted` handler
  - [x] `frontend/src/components/ToolCallMessage.svelte`:
    - Show MCP server name in header (e.g., "MCP: filesystem - read_file")
    - Collapse entire tool call by default (add `let expanded = false` state)
    - When collapsed, show only header with status icon, MCP name, tool name
    - When expanded, show arguments and result sections

---

## Remaining Work Summary

All implementation complete. Only manual testing with actual MCP servers remains.

---

## File Changes Summary

### Backend (Rust) - COMPLETED
| File | Changes | Status |
|------|---------|--------|
| `packages/rhd_ai/src/client.rs` | Add reasoning_content to StreamDelta, StreamChunk | ✅ |
| `packages/rhd_db/src/chat_db.rs` | Add thinking_content column, update Message struct | ✅ |
| `packages/rhd_api/src/lib.rs` | Add ChatThinkingChunkEvent, update ChatMessageDto, ToolCallStartedEvent | ✅ |
| `packages/rhd_app/src/chat.rs` | Emit MessageAdded for system prompts, add mcp_name to ToolCallStarted, emit thinking chunks, store thinking | ✅ |
| `packages/rhd_app/src/ws.rs` | Handle ChatThinkingChunk event, pass mcp_name in ToolCallStarted | ✅ |
| `packages/rhd_app/src/project_manager.rs` | Return mcp_name from get_mcp_clients | ✅ |

### Frontend (TypeScript/Svelte)
| File | Changes | Status |
|------|---------|--------|
| `src/lib/types/index.ts` | Add thinkingContent to ChatMessage | ✅ |
| `src/lib/types/index.ts` | Add mcpName to ToolCall | ✅ |
| `src/lib/types/ws.ts` | Add ChatThinkingChunkEventSchema | ✅ |
| `src/lib/types/ws.ts` | Update ToolCallStartedEventSchema with mcpName | ✅ |
| `src/lib/chatStores.ts` | Add streamingThinkingContent store | ✅ |
| `src/lib/chatWs.ts` | Handle chatThinkingChunk event | ✅ |
| `src/lib/chatWs.ts` | Pass mcpName in toolCallStarted handler | ✅ |
| `src/components/Message.svelte` | Add system prompt and thinking collapsible sections | ✅ |
| `src/components/StreamingMessage.svelte` | Show thinking while streaming | ✅ |
| `src/components/MessageList.svelte` | Pass thinkingContent to StreamingMessage | ✅ |
| `src/components/ToolCallMessage.svelte` | Add MCP name, collapse by default | ✅ |

---

## Testing Considerations
- System prompts: Verify event emitted, collapsible behavior, content displays correctly
- Thinking: Test with Qwen model that supports reasoning_content field
- MCP calls: Verify MCP name displayed, collapsed by default

## Migration Notes
- Database migration for thinking_content column is automatic (like existing migrations)
- Existing messages will have NULL thinking_content (handled gracefully)
