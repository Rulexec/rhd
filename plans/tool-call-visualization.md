# Tool Call Visualization Plan

## Overview
Implement visualization of tool calls in assistant messages on the frontend, showing arguments and results in a collapsible UI for both streaming and persisted messages.

## Current State Analysis

### Backend Data Model
- [`Message`](packages/rhd_chat_api/src/common.rs:63) has:
  - `tool_calls: Vec<ToolCall>` - tool calls made by assistant
  - `tool_call_id: Option<String>` - for tool result messages, links to the tool call
- [`ToolCall`](packages/rhd_chat_api/src/tools.rs:63) has:
  - `id: String` - unique identifier
  - `call_type: String` - always "function"
  - `function: FunctionCall` - name and arguments
  - `tags: Vec<String>` - tags attached to tool call
- Tool results are stored as separate messages with `role="tool"` and `tool_call_id`

### Frontend Gaps
- [`MessageSchema`](frontend/src/lib/api/schemas.ts:24) missing `toolCalls` and `toolCallId` fields
- No `ToolCall` or `FunctionCall` schemas defined
- [`Message.svelte`](frontend/src/lib/components/Message.svelte:192) has basic streaming tool call display (name + arguments only)
- No visualization for persisted tool calls
- No tool result message display

## Implementation Plan

### Phase 1: Schema Updates
**File: `frontend/src/lib/api/schemas.ts`**

1. Add `FunctionCallSchema`:
```typescript
export const FunctionCallSchema = z.object({
  name: z.string(),
  arguments: z.string()
});
```

2. Add `ToolCallSchema`:
```typescript
export const ToolCallSchema = z.object({
  id: z.string(),
  type: z.string(),
  function: FunctionCallSchema,
  tags: z.array(z.string()).default([])
});
```

3. Update `MessageSchema` to include:
```typescript
toolCallId: z.string().optional(),
toolCalls: z.array(ToolCallSchema).default([])
```

### Phase 2: ToolCallMessage Component
**File: `frontend/src/lib/components/ToolCallMessage.svelte`**

Create a new collapsible component with:
- **Header**: Tool name + MCP ID (if namespaced like `fs1/read_file`)
- **Expanded view**:
  - Arguments section (formatted JSON)
  - Result section (from linked tool result message)
- **Props**:
  - `toolCall: ToolCall | StreamToolCallDelta`
  - `result?: string` (tool result content)

### Phase 3: Message.svelte Updates
**File: `frontend/src/lib/components/Message.svelte`**

1. Import and use `ToolCallMessage` component
2. For streaming messages:
   - Pass `streamContent.toolCalls` to ToolCallMessage
3. For persisted messages:
   - Use `message.toolCalls` array
   - Find linked tool result messages
4. Handle tool result messages (`role="tool"`):
   - Display as collapsible "Tool Result" section
   - Link to parent tool call via `toolCallId`

### Phase 4: ChatStore Updates
**File: `frontend/src/stores/ChatStore.ts`**

Add helper to find tool result message by `toolCallId`:
```typescript
getToolResult(toolCallId: string): string | null
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Streaming Flow                            │
├─────────────────────────────────────────────────────────────────┤
│  streamChunk (toolCallDelta) → ChatStore.streamingToolCalls     │
│                              → Message.svelte                   │
│                              → ToolCallMessage                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       Persisted Flow                             │
├─────────────────────────────────────────────────────────────────┤
│  message.toolCalls → Message.svelte                             │
│                    → ToolCallMessage                            │
│  message (role=tool) → Tool Result section                      │
│                      → linked via toolCallId                    │
└─────────────────────────────────────────────────────────────────┘
```

## Files to Modify

| File | Changes |
|------|---------|
| `frontend/src/lib/api/schemas.ts` | Add FunctionCallSchema, ToolCallSchema; update MessageSchema |
| `frontend/src/lib/components/ToolCallMessage.svelte` | New component |
| `frontend/src/lib/components/Message.svelte` | Use ToolCallMessage, handle tool results |
| `frontend/src/stores/ChatStore.ts` | Add helper method to find tool results |

## Testing Considerations

1. **Schema tests**: Verify toolCalls serialization/deserialization
2. **Component tests**: Mock tool calls with arguments and results
3. **Integration**: Verify streaming tool calls transition to persisted correctly
4. **Edge cases**: 
   - Tool calls without results (orphaned)
   - Tool results without matching tool calls
   - Multiple tool calls in single message
