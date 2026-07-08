# Fix Tool Call Visibility and Ordering

## Problem Statement

Two issues with tool call display in chat:

1. **During streaming**: Tool calls not visible in message list
   - User sees: user message → assistant message (with reasoning and final result)
   - Expected: user message → AI reasoning → tool request & response → AI response

2. **After page reload**: Wrong order
   - Current: user message → tool call params → tool call response → assistant message (with reasoning)
   - Expected: user message → AI reasoning → tool call params → tool call response → assistant message

## Root Cause Analysis

### Backend (`packages/rhd_chat/src/tools.rs`)

- Intermediate assistant messages (containing tool calls) stored as JSON: `{"content": "...", "toolCalls": [...]}`
- **Thinking content NOT saved** with intermediate messages
- Thinking content only saved with final assistant message
- DB order: user → assistant (toolCalls JSON, no thinking) → tool (result) → assistant (final with thinking)

### Frontend (`frontend/src/lib/chatWs.ts`)

- `chatToolCallStarted` handler adds tool calls to `streamingMessageId`
- But `streamingMessageId` only set on first `chatStreamChunk`
- Tool calls happen BEFORE final stream chunks (during non-streaming API calls)
- Result: tool calls added to non-existent message, lost

## Solution

### 1. Backend: Save thinking with intermediate messages

**File**: `packages/rhd_chat/src/tools.rs`

In `tool_loop()`, when saving intermediate assistant message with tool calls:
- Extract accumulated thinking content
- Save it with the intermediate message (in JSON structure or separate field)

```rust
// Current code (line ~180):
let assistant_content: String = accumulated_content.lock().await.clone();
let assistant_msg_content = serde_json::json!({
    "content": assistant_content,
    "toolCalls": result.tool_calls,
})
.to_string();
manager.db().add_message(
    chat_id,
    "assistant",
    &assistant_msg_content,
    Some(model),
    None,  // ← thinking_content is None
)?;

// Fixed code:
let assistant_content: String = accumulated_content.lock().await.clone();
let thinking_content: String = accumulated_thinking.lock().await.clone();
let assistant_msg_content = serde_json::json!({
    "content": assistant_content,
    "toolCalls": result.tool_calls,
})
.to_string();
let thinking_option = if thinking_content.is_empty() {
    None
} else {
    Some(thinking_content.as_str())
};
manager.db().add_message(
    chat_id,
    "assistant",
    &assistant_msg_content,
    Some(model),
    thinking_option,  // ← save thinking
)?;
```

### 2. Frontend: Create assistant message on thinking/tool call start

**File**: `frontend/src/lib/chatWs.ts`

**Option A**: Create message on first `chatThinkingChunk`
- When `chatThinkingChunk` arrives and no `streamingMessageId` exists
- Create optimistic assistant message with thinking content
- Set `streamingMessageId`

**Option B**: Create message on `chatToolCallStarted`
- When `chatToolCallStarted` arrives and no `streamingMessageId` exists
- Create optimistic assistant message
- Set `streamingMessageId`
- Add tool call to it

**Recommended**: Option B (simpler, tool calls are the key event)

```typescript
// In chatToolCallStarted handler:
case 'chatToolCallStarted': {
  const { toolCallId, toolName, arguments: args, mcpId } = data as {...};
  const newToolCall = {...};
  pendingToolCalls.update((list) => [...list, newToolCall]);
  
  let tempId = get(streamingMessageId);
  
  // Create assistant message if not exists
  if (!tempId) {
    const chatId = get(currentChatId);
    if (!chatId) break;
    
    tempId = `temp-${Date.now()}`;
    const assistantMessage = {
      id: tempId,
      chatId,
      role: 'assistant' as const,
      content: '',
      createdAt: new Date().toISOString(),
      model: get(selectedModel) || '',
      thinkingContent: get(streamingThinkingContent) || undefined,
      toolCalls: [newToolCall],
    };
    messages.update((list) => [...list, assistantMessage as any]);
    streamingMessageId.set(tempId);
  } else {
    // Add tool call to existing message
    messages.update((list) =>
      list.map((m) => {
        if (m.id === tempId) {
          const existingToolCalls = m.toolCalls || [];
          return { ...m, toolCalls: [...existingToolCalls, newToolCall] };
        }
        return m;
      })
    );
  }
  break;
}
```

### 3. Update test scenario

**File**: `tests/cases/chat-mcp-tools.md`

Update "Expected Results" and "Events Flow" to reflect:
- Tool calls visible during streaming
- Thinking content appears before tool calls
- Proper ordering after reload

### 4. Update E2E test

**File**: `frontend/src/tests/e2e/chat-mcp-tools.test.ts`

Add assertions:
- Tool call messages visible in message list during streaming
- Thinking content appears before tool calls
- After reload, message order preserved correctly

## Implementation Steps

1. **Backend** (`packages/rhd_chat/src/tools.rs`):
   - Save thinking_content with intermediate assistant messages

2. **Frontend** (`frontend/src/lib/chatWs.ts`):
   - Create assistant message on `chatToolCallStarted` if not exists
   - Ensure tool calls added to correct message

3. **Test scenario** (`tests/cases/chat-mcp-tools.md`):
   - Update expected results and event flow

4. **E2E test** (`frontend/src/tests/e2e/chat-mcp-tools.test.ts`):
   - Add tool call visibility assertions
   - Add ordering assertions after reload

## Expected Behavior After Fix

### During Streaming

1. User sends message
2. AI starts thinking → thinking content streams
3. AI decides to call tool → assistant message created with thinking
4. Tool call appears under assistant message
5. Tool executes → tool result appears
6. AI continues → final response streams
7. Final assistant message updated with complete response

### After Page Reload

Message order in DB and UI:
1. User message
2. Assistant message (thinking + tool calls)
3. Tool result message
4. Assistant message (final response with thinking)

## Files to Modify

- `packages/rhd_chat/src/tools.rs` (backend)
- `frontend/src/lib/chatWs.ts` (frontend)
- `tests/cases/chat-mcp-tools.md` (test scenario)
- `frontend/src/tests/e2e/chat-mcp-tools.test.ts` (E2E test)

## Implementation Details

### Backend Changes (`packages/rhd_chat/src/tools.rs`)

1. **Save thinking content with intermediate messages**:
   - Extract `accumulated_thinking` when saving intermediate assistant message
   - Pass thinking content to `add_message()` instead of `None`

2. **Emit MessageAdded events for intermediate messages**:
   - Emit `MessageAdded` for intermediate assistant message (with toolCalls)
   - Emit `MessageAdded` for each tool result message
   - Save intermediate assistant message BEFORE tool messages to ensure correct DB ordering

### Frontend Changes (`frontend/src/lib/chatWs.ts`)

1. **Create assistant message on ToolCallStarted**:
   - When `chatToolCallStarted` arrives and no `streamingMessageId` exists
   - Create optimistic assistant message with toolCalls
   - Set `streamingMessageId` to track the message

2. **Parse JSON content for assistant messages**:
   - When `chatMessageAdded` arrives for assistant message
   - Parse JSON content to extract toolCalls
   - Normalize toolCalls from OpenAI format (`function.name`, `function.arguments`) to frontend format (`name`, `arguments`)

3. **Merge tool results into assistant messages**:
   - When `chatMessageAdded` arrives for tool result message
   - Parse tool result JSON to extract `toolCallId` and `result`
   - Find assistant message with matching toolCallId
   - Update toolCall with result and status='completed'

4. **Reorder messages based on numeric IDs**:
   - When replacing temp message with real message
   - Remove temp message and insert real message at correct position
   - Use `getNumericId()` helper to handle both numeric and string IDs
   - Temp messages (string IDs) treated as `Number.MAX_SAFE_INTEGER`

5. **Handle message ordering correctly**:
   - Insert messages in correct position based on ID
   - Ensure intermediate assistant message appears before tool results
   - Ensure tool results appear before final assistant message

### Test Updates

1. **Test scenario** (`tests/cases/chat-mcp-tools.md`):
   - Updated event flow to include `ThinkingChunk` events
   - Updated expected results to verify tool call visibility
   - Documented message ordering after reload

2. **E2E test** (`frontend/src/tests/e2e/chat-mcp-tools.test.ts`):
   - Added assertions to verify tool calls visible in messages
   - Added assertions to verify tool call status and result
   - Added assertions to verify message ordering (user → toolCallMsg → toolResult → finalAssistant)
   - Updated to find final assistant message (last one) instead of first one

## Status

✅ **Completed** - All changes implemented and tested successfully.
