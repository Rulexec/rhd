# Fix: Clear streamingThinkingContent after streaming completes

## Problem

The `streamingThinkingContent` store accumulates thinking/reasoning content during AI streaming but is not cleared when:
1. Streaming finishes successfully (`chatStreamFinished` event)
2. Streaming encounters an error (`chatStreamError` event)
3. User switches to a different chat (`selectChat()`)

This causes the thinking content from previous messages to persist in the exported state and potentially affect subsequent interactions.

## Root Cause

In [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts):
- `streamingThinkingContent` is correctly cleared at the **start** of `sendMessage()` (line 136) and `editMessage()` (line 165)
- But it's **not cleared** at the **end** of streaming or when switching chats

## Solution

Add `streamingThinkingContent.set('')` in three locations:

### 1. `chatStreamFinished` handler (line 250-253)
```typescript
case 'chatStreamFinished': {
  isStreaming.set(false);
  streamError.set(null);
  streamingThinkingContent.set('');  // ADD THIS
  break;
}
```

### 2. `chatStreamError` handler (line 255-260)
```typescript
case 'chatStreamError': {
  const error = data as { error: string };
  streamError.set(error.error);
  isStreaming.set(false);
  streamingMessageId.set(null);
  streamingThinkingContent.set('');  // ADD THIS
  break;
}
```

### 3. `selectChat()` function (line 68-84)
```typescript
export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(response.data.messages || []);
    streamingContent.set('');
    streamingThinkingContent.set('');  // ADD THIS
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    chatProjects.set([]);
    mcpStatuses.set([]);
    loadChatProjects(chatId);
  }
  return response;
}
```

## Files to Modify

- [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts) - Add three lines to clear `streamingThinkingContent`

## Testing

After the fix, the exported state should show:
```json
{
  "streamingThinkingContent": "",
  "isStreaming": false
}
```

Both during normal operation and after calling `__exportState()`.

## Related Code

- [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts:9) - Store definition
- [`frontend/src/lib/stateExport.ts`](frontend/src/lib/stateExport.ts:76) - Export includes `streamingThinkingContent`
- [`frontend/src/lib/chatWs.ts:238`](frontend/src/lib/chatWs.ts:238) - Where content is accumulated
