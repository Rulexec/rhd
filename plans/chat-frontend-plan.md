# Chat Frontend Plan

## Goal

Implement chat UI in Svelte frontend: chat list, chat view with messages, streaming display, input with send/abort/retry, and edit functionality.

## Scope

- Chat stores for state management
- WebSocket client functions for chat operations
- Chat list sidebar with create/delete
- Chat view with message list and input
- Streaming message display
- Edit message with resend
- Keyboard shortcuts (Enter to send, Shift+Enter for newline)

## Architecture

```
ChatsTab
├── ChatList (sidebar)
│   ├── New Chat button
│   └── Chat items (click to select, delete button)
└── ChatView (main area)
    ├── Header (chat title)
    ├── MessageList
    │   ├── Message (user/assistant)
    │   └── StreamingMessage (when active)
    └── MessageInput (textarea + send/abort button)
```

## Stores

```javascript
// frontend/src/lib/chatStores.js
import { writable, derived } from 'svelte/store';

export const chats = writable([]); // Array of { id, title, createdAt, updatedAt }
export const currentChatId = writable(null); // Selected chat ID
export const messages = writable([]); // Messages for current chat
export const streamingContent = writable(''); // Partial AI response
export const isStreaming = writable(false); // Streaming active
export const streamError = writable(null); // Error message if failed

export const currentChat = derived(
  [chats, currentChatId],
  ([$chats, $currentChatId]) => $chats.find(c => c.id === $currentChatId)
);
```

## WebSocket Functions

```javascript
// frontend/src/lib/chatWs.js
import { chats, messages, streamingContent, isStreaming, streamError, currentChatId } from './chatStores.js';

export async function createChat(title) {
  // Send createChat request
  // On success: add to chats store, select new chat
}

export async function loadChats() {
  // Send listChats request
  // On success: set chats store
}

export async function selectChat(chatId) {
  // Send getChat request
  // On success: set currentChatId, set messages store
}

export async function deleteChat(chatId) {
  // Send deleteChat request
  // On success: remove from chats store, clear selection if deleted
}

export async function sendMessage(content, model) {
  // Add user message to messages store immediately (optimistic)
  // Set isStreaming = true, clear streamError
  // Send sendMessage request
  // Streaming updates handled by event handler
}

export async function editMessage(messageId, newContent, model) {
  // Update message in store immediately (optimistic)
  // Truncate messages after edited message in store
  // Set isStreaming = true, clear streamError
  // Send editMessage request
}

export async function abortChat() {
  // Send abortChat request
}

// Event handler (called from ws.js)
export function handleChatEvent(event, data) {
  switch (event) {
    case 'chatstreamchunk':
      streamingContent.update(c => c + data.content);
      break;
    case 'chatstreamfinished':
      // Add assistant message to messages store
      // Clear streaming state
      isStreaming.set(false);
      streamingContent.set('');
      break;
    case 'chatstreamerror':
      streamError.set(data.error);
      isStreaming.set(false);
      break;
    case 'chatmessageadded':
      // Add message to store if not already present (handle optimistic updates)
      break;
  }
}
```

## Components

### ChatsTab.svelte

Layout: two-column (sidebar + main)

```svelte
<div class="chats-tab">
  <ChatList />
  {#if $currentChatId}
    <ChatView />
  {:else}
    <div class="placeholder">Select or create a chat</div>
  {/if}
</div>

<style>
  .chats-tab {
    display: flex;
    height: calc(100vh - 100px);
  }
</style>
```

### ChatList.svelte

Sidebar with chat list

```svelte
<div class="chat-list">
  <button on:click={createNewChat}>+ New Chat</button>
  <div class="chats">
    {#each $chats as chat (chat.id)}
      <div
        class="chat-item"
        class:selected={chat.id === $currentChatId}
        on:click={() => selectChat(chat.id)}
      >
        <span class="title">{chat.title}</span>
        <button class="delete" on:click|stopPropagation={() => deleteChat(chat.id)}>×</button>
      </div>
    {/each}
  </div>
</div>
```

### ChatView.svelte

Main chat area

```svelte
<div class="chat-view">
  <div class="header">
    <h2>{$currentChat.title}</h2>
  </div>
  <MessageList />
  <MessageInput />
</div>
```

### MessageList.svelte

Scrollable message list

```svelte
<div class="message-list" bind:this={listElement}>
  {#each $messages as message (message.id)}
    <Message {message} />
  {/each}
  {#if $isStreaming}
    <StreamingMessage content={$streamingContent} />
  {/if}
</div>

<script>
  import { onMount, afterUpdate } from 'svelte';
  import { messages, isStreaming, streamingContent } from '../lib/chatStores.js';
  import Message from './Message.svelte';
  import StreamingMessage from './StreamingMessage.svelte';

  let listElement;

  afterUpdate(() => {
    // Auto-scroll to bottom
    if (listElement) {
      listElement.scrollTop = listElement.scrollHeight;
    }
  });
</script>
```

### Message.svelte

Individual message

```svelte
<div class="message {message.role}">
  {#if editing}
    <textarea bind:value={editContent}></textarea>
    <button on:click={saveEdit}>Save</button>
    <button on:click={() => editing = false}>Cancel</button>
  {:else}
    <div class="content">{message.content}</div>
    {#if message.role === 'user'}
      <button class="edit" on:click={startEdit}>Edit</button>
    {/if}
  {/if}
</div>

<script>
  export let message;
  let editing = false;
  let editContent = message.content;

  function startEdit() {
    editing = true;
    editContent = message.content;
  }

  function saveEdit() {
    // Call editMessage(message.id, editContent, model)
    editing = false;
  }
</script>
```

### MessageInput.svelte

Input area with send/abort

```svelte
<div class="message-input">
  {#if $streamError}
    <div class="error">
      <span>{$streamError}</span>
      <button on:click={retry}>Retry</button>
    </div>
  {/if}
  <textarea
    bind:value={input}
    on:keydown={handleKeydown}
    placeholder="Type a message..."
    disabled={$isStreaming}
  ></textarea>
  {#if $isStreaming}
    <button on:click={abort} class="abort">Abort</button>
  {:else}
    <button on:click={send} disabled={!input.trim()}>Send</button>
  {/if}
</div>

<script>
  import { isStreaming, streamError, currentChatId } from '../lib/chatStores.js';
  import { sendMessage, abortChat, editMessage } from '../lib/chatWs.js';

  let input = '';
  let model = 'gpt4'; // TODO: model selector

  function handleKeydown(event) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }

  function send() {
    if (!input.trim() || $isStreaming) return;
    sendMessage(input, model);
    input = '';
  }

  function abort() {
    abortChat();
  }

  function retry() {
    // Retry last failed message
    streamError.set(null);
    // Resend logic
  }
</script>
```

### StreamingMessage.svelte

Streaming response display

```svelte
<div class="message assistant streaming">
  <div class="content">{content}</div>
  {#if !content}
    <div class="loading">...</div>
  {/if}
</div>

<script>
  export let content;
</script>
```

## WebSocket Integration

Update `frontend/src/lib/ws.js` to handle chat events:

```javascript
import { handleChatEvent } from './chatWs.js';

function handleEvent(message) {
  const { event, data } = message;

  // Existing scenario events
  if (event.startsWith('scenario') || event.startsWith('step')) {
    // ... existing logic
  }
  // New chat events
  else if (event.startsWith('chat')) {
    handleChatEvent(event, data);
  }
}
```

## File Changes

| File | Change |
|------|--------|
| `frontend/src/lib/chatStores.js` | **NEW** — Chat state stores |
| `frontend/src/lib/chatWs.js` | **NEW** — Chat WebSocket functions and event handler |
| `frontend/src/lib/ws.js` | Update `handleEvent()` to route chat events |
| `frontend/src/components/ChatsTab.svelte` | Replace placeholder with chat UI layout |
| `frontend/src/components/ChatList.svelte` | **NEW** — Chat list sidebar |
| `frontend/src/components/ChatView.svelte` | **NEW** — Main chat view |
| `frontend/src/components/MessageList.svelte` | **NEW** — Message list with auto-scroll |
| `frontend/src/components/Message.svelte` | **NEW** — Individual message with edit |
| `frontend/src/components/MessageInput.svelte` | **NEW** — Input with send/abort/retry |
| `frontend/src/components/StreamingMessage.svelte` | **NEW** — Streaming response display |

## Implementation Steps

1. Create `chatStores.js` with all stores
2. Create `chatWs.js` with WebSocket functions and event handler
3. Update `ws.js` to route chat events to `handleChatEvent()`
4. Build `ChatList.svelte`:
   - Display chat list
   - New chat button
   - Delete button
   - Click to select
5. Build `ChatView.svelte`:
   - Header with title
   - MessageList
   - MessageInput
6. Build `MessageList.svelte`:
   - Render messages
   - Auto-scroll
   - Show streaming message
7. Build `Message.svelte`:
   - Display user/assistant messages
   - Edit mode for user messages
8. Build `MessageInput.svelte`:
   - Textarea with auto-resize
   - Enter to send, Shift+Enter for newline
   - Send button (disabled when empty/streaming)
   - Abort button when streaming
   - Error display with retry
9. Build `StreamingMessage.svelte`:
   - Display partial content
   - Loading indicator when empty
10. Update `ChatsTab.svelte` to compose all components
11. Add CSS styles for all components
12. Test end-to-end flow

## Design Notes

- Optimistic updates: add user message to store immediately before server confirmation
- Deduplication: when `chatmessageadded` event received, check if message already in store
- Auto-scroll: use `afterUpdate()` to scroll to bottom on new content
- Model selector: hardcode for now, can add dropdown later
- Chat title: auto-generate from first message (future enhancement)
- Error recovery: retry button resends last message
- Abort: cancels stream, shows error state with retry option

## Success Criteria

- [ ] Chat list displays and updates
- [ ] Can create new chat
- [ ] Can delete chat
- [ ] Can select chat and view messages
- [ ] Can send message and see streaming response
- [ ] Can abort streaming request
- [ ] Can edit message and resend
- [ ] Can retry failed requests
- [ ] Enter sends, Shift+Enter inserts newline
- [ ] Auto-scroll works during streaming
- [ ] Error states display correctly
- [ ] UI responsive during streaming

## Dependencies

- [chat-api-protocol-plan.md](chat-api-protocol-plan.md) — WebSocket protocol contract
- [chat-backend-plan.md](chat-backend-plan.md) — Backend implementation
