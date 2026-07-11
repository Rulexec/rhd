# Fix: Thinking Content Prepend Bug in Tool Loop

## Problem

When in chat with an attached project with MCP tools, after a tool call completes and the assistant starts streaming again, the new assistant message shows the thinking content from the previous message prepended to the new thinking content.

**Visual symptom:**
1. User asks assistant to call a tool
2. Assistant thinks (thinking content A shown)
3. Tool call executes
4. Assistant thinks again - but shows "A + B" instead of just "B"
5. When streaming ends, it gets replaced with actual reasoning content "B" (correct)

The behavior works correctly but visualizes wrongly.

## Root Cause

In [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts), the `streamingThinkingContent` store accumulates thinking/reasoning content during AI streaming but is **not cleared** when an intermediate assistant message with tool calls is added.

### Flow Analysis

1. User sends message → assistant starts streaming with thinking content
2. `chatThinkingChunk` events accumulate thinking in `streamingThinkingContent` store (line 239)
3. Assistant makes a tool call → `chatMessageAdded` event arrives for intermediate assistant message
4. In the handler (line 332-384), the temp message is replaced with real message
5. `streamingMessageId` is set to null (line 379)
6. **BUT `streamingThinkingContent` is NOT cleared!**
7. Tool executes, next iteration starts
8. New `chatToolCallStarted` event arrives (line 448-499)
9. When creating a new temp message (line 479), it uses `get(streamingThinkingContent)` which still has the OLD thinking content!
10. Same issue in `chatStreamChunk` handler (line 224)

### Code Locations

**Problem 1: `chatMessageAdded` handler (line 332-384)**
```typescript
// Handle assistant message: replace temp message if exists
if (added.message.role === 'assistant' && tempId) {
  // ... replace temp message ...
  streamingMessageId.set(null);  // ← streamingMessageId cleared
  // ← BUT streamingThinkingContent NOT cleared!
  return updated;
}
```

**Problem 2: `chatToolCallStarted` handler (line 479)**
```typescript
const assistantMessage = {
  // ...
  thinkingContent: get(streamingThinkingContent) || undefined,  // ← Uses stale content!
  toolCalls: [newToolCall],
};
```

**Problem 3: `chatStreamChunk` handler (line 224)**
```typescript
const assistantMessage = {
  // ...
  thinkingContent: get(streamingThinkingContent) || undefined,  // ← Uses stale content!
};
```

## Solution

Clear `streamingThinkingContent` when handling `chatMessageAdded` for an assistant message that has tool calls (intermediate message in tool loop).

### Changes to [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

**In `chatMessageAdded` handler (around line 379):**
```typescript
// Handle assistant message: replace temp message if exists
if (added.message.role === 'assistant' && tempId) {
  const tempIdx = list.findIndex((m) => m.id === tempId);
  if (tempIdx !== -1) {
    // ... existing code to parse and replace message ...
    
    streamingMessageId.set(null);
    streamingThinkingContent.set('');  // ← ADD THIS: Clear thinking content for next iteration
    return updated;
  } else {
    streamingMessageId.set(null);
  }
}
```

## Files to Modify

- [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts) - Add one line to clear `streamingThinkingContent` in `chatMessageAdded` handler

## Testing

After the fix:
1. Send a message that triggers a tool call
2. Observe assistant thinking content during first iteration
3. After tool call completes, observe that new thinking content starts fresh (not prepended with previous thinking)
4. Final message should show only the thinking from that iteration

## Related Plans

- [`plans/fix-streaming-thinking-content-clear.md`](plans/fix-streaming-thinking-content-clear.md) - Previous fix for clearing thinking content on stream finish/error/chat switch
