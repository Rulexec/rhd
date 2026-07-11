# Phase 6: Frontend - Chat Tool Execution UI

## Goal
Display tool calls, implement control buttons (cancel/pause/resume).

## Current State Analysis
- [`MessageList.svelte`](frontend/src/components/MessageList.svelte) renders chat messages
- [`MessageInput.svelte`](frontend/src/components/MessageInput.svelte) has send button and streaming state
- [`chatStores.ts`](frontend/src/lib/chatStores.ts) has `isStreaming`, `streamingContent`, `streamError`
- [`chatWs.ts`](frontend/src/lib/chatWs.ts) handles chat events
- [`ChatMessage`](frontend/src/lib/types/index.ts:47) has `role: 'user' | 'assistant'` - need to extend

## Subtasks

### 6.1. Extend message types for tool calls
**File**: [`frontend/src/lib/types/index.ts`](frontend/src/lib/types/index.ts)

**Extend ChatMessageSchema**:
```typescript
export const ChatMessageSchema = z.object({
  id: z.union([z.number(), z.string()]),
  chatId: z.number(),
  role: z.enum(['user', 'assistant', 'system', 'tool']),
  content: z.string(),
  createdAt: z.string(),
  model: z.string().nullable(),
  toolCalls: z.array(z.object({
    id: z.string(),
    name: z.string(),
    arguments: z.string(),
    result: z.string().optional(),
    status: z.enum(['pending', 'running', 'completed', 'failed']).optional(),
  })).optional(),
  toolCallId: z.string().optional(),
});
```

**New type**:
```typescript
export const ToolCallSchema = z.object({
  id: z.string(),
  name: z.string(),
  arguments: z.string(),
  result: z.string().optional(),
  status: z.enum(['pending', 'running', 'completed', 'failed']).optional(),
});
export type ToolCall = z.infer<typeof ToolCallSchema>;
```

### 6.2. Create ToolCallMessage component
**File**: `frontend/src/components/ToolCallMessage.svelte` (new)

**UI**:
- Display tool call with:
  - Tool name (header)
  - Arguments (collapsible, formatted as JSON)
  - Result (collapsible, formatted)
  - Status indicator (spinner for running, checkmark for completed, X for failed)

**Props**:
```typescript
interface Props {
  toolCall: ToolCall;
}
```

**Styling**:
- Compact view by default
- Expandable sections for arguments/result
- Color-coded status

### 6.3. Update MessageList for tool calls
**File**: [`frontend/src/components/MessageList.svelte`](frontend/src/components/MessageList.svelte)

**Changes**:
- Render tool call messages inline with assistant messages
- Group tool calls with their results
- Show tool calls as they happen (real-time updates)

**Logic**:
- If message has `toolCalls` array, render each with `ToolCallMessage` component
- Tool calls appear after assistant message that requested them
- Tool results appear as separate messages with `role: 'tool'`

### 6.4. Add control buttons to MessageInput
**File**: [`frontend/src/components/MessageInput.svelte`](frontend/src/components/MessageInput.svelte)

**New buttons**:
- **Cancel** (visible when streaming/tool loop running) - calls `abortChat()`
- **Pause** (visible when tool loop running, not paused) - calls `pauseChat()`
- **Resume** (visible when paused) - calls `resumeChat()`

**Disable send when**:
- Streaming in progress (existing behavior)
- Any MCP server not connected (new)

**Enable send during pause**:
- User can send message to guide AI
- Message appended to history, tool loop resumes

**UI layout**:
- Replace send button with control buttons when active
- Or show control buttons alongside send button

### 6.5. Add WebSocket handlers for tool events
**File**: [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

**New functions**:
```typescript
export async function pauseChat(): Promise<WsResponse>
export async function resumeChat(): Promise<WsResponse>
```

**Event handling** in `handleChatEvent()`:
- `toolCallStarted` → add pending tool call to message
  ```typescript
  const { chatId, toolCallId, toolName, arguments: args } = data;
  // Find or create assistant message with tool calls
  // Add tool call with status: 'running'
  ```
- `toolCallCompleted` → update tool call with result
  ```typescript
  const { chatId, toolCallId, result } = data;
  // Find tool call by ID, update result and status: 'completed'
  ```
- `chatPaused` → update UI state
  ```typescript
  isPaused.set(true);
  isStreaming.set(false);  // or separate state
  ```
- `chatResumed` → update UI state
  ```typescript
  isPaused.set(false);
  isStreaming.set(true);
  ```

### 6.6. Update chat stores for tool execution
**File**: [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts)

**New stores**:
```typescript
export const isPaused = writable<boolean>(false);
export const pendingToolCalls = writable<ToolCall[]>([]);
```

**Update `isStreaming`**:
- Include tool execution state (not just AI streaming)
- `isStreaming = true` when AI streaming OR tool loop running

**New derived stores**:
```typescript
export const isToolLoopRunning = derived(
  [isStreaming, isPaused],
  ([$isStreaming, $isPaused]) => $isStreaming && !$isPaused
);
```

## Deliverables
- [ ] Extended message types in [`frontend/src/lib/types/index.ts`](frontend/src/lib/types/index.ts)
- [ ] ToolCallMessage component
- [ ] Tool calls in [`MessageList.svelte`](frontend/src/components/MessageList.svelte)
- [ ] Control buttons (cancel/pause/resume) in [`MessageInput.svelte`](frontend/src/components/MessageInput.svelte)
- [ ] WebSocket handlers for tool events in [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)
- [ ] Updated chat stores in [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts)

## Dependencies
- Phase 4 (Backend tool execution) must be complete
- Phase 5 (Frontend project UI) should be complete for full integration

## Risk Assessment
- **Medium risk**: Real-time tool call updates require careful state management
- **Unknown**: How to handle very large tool arguments/results in UI
- **Mitigation**: 
  - Collapsible sections for large content
  - Truncate in UI, full content available on expand
  - Virtual scrolling if many tool calls
