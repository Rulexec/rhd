# Fix: Chat Projects Store Not Cleared on Chat Switch

## Problem

When creating new chat or switching between chats, the `chatProjects` store retains projects from previous chat. This causes:
- New chat shows projects attached to previous chat
- System prompts not applied (backend correctly has no projects for new chat)
- After page refresh, projects disappear (correct state loaded from DB)

## Root Cause

[`chatProjects`](frontend/src/lib/projectStores.ts:3) is global Svelte store not scoped to current chat.

**[`createChat()`](frontend/src/lib/chatWs.ts:43)** clears `messages` and `selectedModel` but NOT `chatProjects` or `mcpStatuses`.

**[`selectChat()`](frontend/src/lib/chatWs.ts:68)** calls `loadChatProjects()` but doesn't clear old data first, causing race condition where old projects briefly visible.

## Backend Analysis

Backend is correct:
- [`chat_projects`](packages/rhd_db/src/chat_db.rs:72) table properly scoped by `chat_id`
- New chat has no projects in DB
- [`get_chat_projects()`](packages/rhd_db/src/chat_db.rs:289) returns only projects for specific chat
- System prompt injection logic works correctly

## Fix

### 1. Clear `chatProjects` in `createChat()`

File: [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

```typescript
export async function createChat(title: string): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'createChat', id, title });
  if (response.success) {
    const chatId = response.data.chatId;
    const newChat = {
      id: chatId,
      title,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      activeModel: null,
    };
    chats.update((list) => [...list, newChat]);
    currentChatId.set(chatId);
    messages.set([]);
    selectedModel.set(null);
    chatProjects.set([]);  // ADD: Clear projects for new chat
    mcpStatuses.set([]);   // ADD: Clear MCP statuses
  }
  return response;
}
```

### 2. Clear `chatProjects` in `selectChat()` before loading

File: [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

```typescript
export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(response.data.messages || []);
    streamingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    chatProjects.set([]);  // ADD: Clear old projects before loading
    mcpStatuses.set([]);   // ADD: Clear old MCP statuses
    loadChatProjects(chatId);
  }
  return response;
}
```

### 3. Import stores in `chatWs.ts`

Add imports at top of [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts):

```typescript
import { chatProjects, mcpStatuses } from './projectStores';
```

## Testing

1. Create new chat → verify no projects shown in UI
2. Attach project to chat → verify project shown
3. Switch to different chat → verify correct projects shown (or none)
4. Switch back to original chat → verify projects still shown
5. Page refresh → verify state persists correctly

## Files to Modify

- [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts) - Add store clearing logic
