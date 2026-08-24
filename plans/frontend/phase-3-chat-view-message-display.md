# Phase 3: Chat View & Message Display

## Overview

Implement the chat view that displays messages for a selected chat, with Markdown rendering, reasoning content collapse/expand, and auto-scrolling behavior. This phase provides the core chat interaction interface.

**Scope:**
- Current chat store with message management
- Chat API methods (get chat, subscribe to chat events)
- Chat view component with message list
- Message component with Markdown rendering and reasoning content
- Auto-scrolling behavior (only when at bottom)
- Tag display on messages

**Out of Scope:**
- Message input / queue messages (Phase 4)
- Connection status UI (Phase 6)
- Plugins tab (Phase 5)

## Dependencies

- **Requires**: Phase 1 (WebSocket foundation), Phase 2 (chat list for selection)
- **Blocks**: Phase 4 (needs message display for queue messages)

## Files to Create/Modify

### 1. `frontend/src/lib/api/chatApi.js`

**Modify:** Add methods for getting chat details and subscribing to chat events.

```javascript
// Add these imports at the top
import {
  // ... existing imports ...
  GetChatResultSchema,
  MessageAddedDataSchema,
  MessageUpdatedDataSchema,
  MessageDeletedDataSchema
} from './schemas.js';

/**
 * Subscribe to chat events for a specific chat.
 * @param {number} chatId - Chat ID to subscribe to
 * @returns {Promise<void>}
 */
export async function subscribeChat(chatId) {
  await websocket.request('subscribeChat', { chatId });
}

/**
 * Unsubscribe from chat events.
 * @param {number} chatId - Chat ID to unsubscribe from
 * @returns {Promise<void>}
 */
export async function unsubscribeChat(chatId) {
  await websocket.request('unsubscribeChat', { chatId });
}

/**
 * Get chat details with messages.
 * @param {number} chatId - Chat ID
 * @returns {Promise<import('./schemas.js').GetChatResult>}
 */
export async function getChat(chatId) {
  const data = await websocket.request('getChat', { chatId });
  return GetChatResultSchema.parse(data);
}

/**
 * Register event listeners for chat-specific events.
 * @param {number} chatId - Chat ID to listen for
 * @param {Object} handlers
 * @param {Function} handlers.onMessageAdded
 * @param {Function} handlers.onMessageUpdated
 * @param {Function} handlers.onMessageDeleted
 * @returns {Function} - Cleanup function
 */
export function onChatEvents(chatId, { onMessageAdded, onMessageUpdated, onMessageDeleted }) {
  const unsubs = [];

  if (onMessageAdded) {
    unsubs.push(websocket.on('messageAdded', (data) => {
      const parsed = MessageAddedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onMessageAdded(parsed);
      }
    }));
  }

  if (onMessageUpdated) {
    unsubs.push(websocket.on('messageUpdated', (data) => {
      const parsed = MessageUpdatedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onMessageUpdated(parsed);
      }
    }));
  }

  if (onMessageDeleted) {
    unsubs.push(websocket.on('messageDeleted', (data) => {
      const parsed = MessageDeletedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onMessageDeleted(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
```

### 2. `frontend/src/lib/stores/chat.js`

**Purpose:** Svelte store for current chat (messages, metadata).

```javascript
import { writable, derived } from 'svelte/store';
import { getChat, subscribeChat, unsubscribeChat, onChatEvents } from '../api/chatApi.js';

/**
 * Current chat ID.
 * @type {import('svelte/store').Writable<number | null>}
 */
export const currentChatId = writable(null);

/**
 * Current chat metadata.
 * @type {import('svelte/store').Writable<import('../api/schemas.js').Chat | null>}
 */
export const currentChat = writable(null);

/**
 * Messages for the current chat.
 * @type {import('svelte/store').Writable<import('../api/schemas.js').Message[]>}
 */
export const messages = writable([]);

/**
 * Loading state.
 * @type {import('svelte/store').Writable<boolean>}
 */
export const chatLoading = writable(false);

/**
 * Error state.
 * @type {import('svelte/store').Writable<string | null>}
 */
export const chatError = writable(null);

/**
 * Cleanup function for event listeners.
 * @type {Function | null}
 */
let cleanupEvents = null;

/**
 * Select a chat and load its messages.
 * @param {number} chatId - Chat ID to select
 */
export async function selectChat(chatId) {
  // Cleanup previous subscription
  await clearChat();

  currentChatId.set(chatId);
  chatLoading.set(true);
  chatError.set(null);

  try {
    // Subscribe to chat events
    await subscribeChat(chatId);

    // Register event listeners
    cleanupEvents = onChatEvents(chatId, {
      onMessageAdded: ({ message }) => {
        messages.update(msgs => {
          // Don't add if already exists
          if (msgs.some(m => m.id === message.id)) {
            return msgs.map(m => m.id === message.id ? message : m);
          }
          return [...msgs, message];
        });
      },
      onMessageUpdated: ({ message }) => {
        messages.update(msgs =>
          msgs.map(m => m.id === message.id ? message : m)
        );
      },
      onMessageDeleted: ({ messageId }) => {
        messages.update(msgs =>
          msgs.filter(m => m.id !== messageId)
        );
      }
    });

    // Load chat data
    const result = await getChat(chatId);
    currentChat.set(result.chat);
    messages.set(result.messages);
  } catch (error) {
    chatError.set(error.message);
  } finally {
    chatLoading.set(false);
  }
}

/**
 * Clear the current chat and unsubscribe.
 */
export async function clearChat() {
  // Cleanup event listeners
  if (cleanupEvents) {
    cleanupEvents();
    cleanupEvents = null;
  }

  // Unsubscribe from previous chat
  const prevChatId = await new Promise(resolve => {
    currentChatId.subscribe(id => resolve(id))();
  });

  if (prevChatId) {
    try {
      await unsubscribeChat(prevChatId);
    } catch (error) {
      console.warn('Failed to unsubscribe from chat:', error);
    }
  }

  currentChatId.set(null);
  currentChat.set(null);
  messages.set([]);
  chatLoading.set(false);
  chatError.set(null);
}

/**
 * Derived store: messages sorted by createdAt ASC.
 */
export const sortedMessages = derived(messages, ($messages) => {
  return [...$messages].sort((a, b) => {
    return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
  });
});
```

### 3. `frontend/src/lib/components/Message.svelte`

**Purpose:** Individual message display component with Markdown rendering and reasoning content.

```svelte
<script>
  import { marked } from 'marked';

  /** @type {import('../api/schemas.js').Message} */
  export let message;

  /** @type {boolean} */
  export let isQueue = false;

  let showMarkdown = true;
  let reasoningExpanded = false;
  let reasoningContentEl = null;

  $: hasReasoning = message.reasoningContent && message.reasoningContent.length > 0;

  /**
   * Render content as Markdown or plain text.
   */
  function renderContent(content) {
    if (!showMarkdown) {
      return content;
    }
    try {
      return marked.parse(content, { breaks: true });
    } catch (error) {
      console.error('Markdown rendering failed:', error);
      return content;
    }
  }

  /**
   * Scroll reasoning content to bottom when collapsed.
   */
  function scrollReasoningToBottom() {
    if (reasoningContentEl && !reasoningExpanded) {
      reasoningContentEl.scrollTop = reasoningContentEl.scrollHeight;
    }
  }

  // Scroll to bottom when reasoning content changes
    if (hasReasoning && !reasoningExpanded) {
      scrollReasoningToBottom();
    }
  });

  function toggleReasoning() {
    reasoningExpanded = !reasoningExpanded;
  }

  function toggleMarkdown() {
    showMarkdown = !showMarkdown;
  }

  function formatTimestamp(dateString) {
    const date = new Date(dateString);
    return date.toLocaleString();
  }

  function getRoleBadgeClass(role) {
    switch (role) {
      case 'user': return 'role-user';
      case 'assistant': return 'role-assistant';
      case 'system': return 'role-system';
      default: return 'role-default';
    }
  }
</script>

<div class="message" class:queue={isQueue}>
  <div class="message-header">
    <div class="message-meta">
      <span class="role-badge {getRoleBadgeClass(message.role)}">
        {message.role}
      </span>
      <span class="timestamp text-muted">
        {formatTimestamp(message.createdAt)}
      </span>
      {#if message.tags && message.tags.length > 0}
        <div class="message-tags">
          {#each message.tags as tag}
            <span class="tag">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>
    <button class="btn-icon markdown-toggle" on:click={toggleMarkdown} title="Toggle Markdown">
      {showMarkdown ? '📄' : '📝'}
    </button>
  </div>

  {#if hasReasoning}
    <div class="reasoning-section">
      <button class="reasoning-toggle" on:click={toggleReasoning}>
        <span class="reasoning-icon">{reasoningExpanded ? '▼' : '▶'}</span>
        <span>Reasoning</span>
      </button>
      <div
        class="reasoning-content"
        class:expanded={reasoningExpanded}
        bind:this={reasoningContentEl}
      >
        {#if showMarkdown}
          {@html renderContent(message.reasoningContent)}
        {:else}
          <pre>{message.reasoningContent}</pre>
        {/if}
      </div>
    </div>
  {/if}

  <div class="message-content">
    {#if showMarkdown}
      {@html renderContent(message.content)}
    {:else}
      <pre>{message.content}</pre>
    {/if}
  </div>
</div>

<style>
  .message {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    position: relative;
  }

  .message.queue {
    opacity: 0.8;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: var(--spacing-sm);
  }

  .message-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    flex-wrap: wrap;
  }

  .role-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px var(--spacing-sm);
    border-radius: var(--radius-full);
    font-size: var(--font-size-xs);
    font-weight: 600;
    text-transform: uppercase;
  }

  .role-user {
    background: var(--color-primary);
    color: white;
  }

  .role-assistant {
    background: var(--color-success);
    color: white;
  }

  .role-system {
    background: var(--color-warning);
    color: white;
  }

  .role-default {
    background: var(--color-bg-tertiary);
    color: var(--color-text-secondary);
  }

  .timestamp {
    font-size: var(--font-size-xs);
  }

  .message-tags {
    display: flex;
    gap: var(--spacing-xs);
  }

  .markdown-toggle {
    font-size: var(--font-size-sm);
    padding: var(--spacing-xs);
  }

  .reasoning-section {
    margin-bottom: var(--spacing-sm);
  }

  .reasoning-toggle {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    padding: var(--spacing-xs) 0;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
  }

  .reasoning-toggle:hover {
    color: var(--color-text);
  }

  .reasoning-icon {
    font-size: var(--font-size-xs);
  }

  .reasoning-content {
    max-height: 4.5em; /* ~3 lines */
    overflow: hidden;
    padding: var(--spacing-sm);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    line-height: 1.5;
    transition: max-height var(--transition-normal);
  }

  .reasoning-content.expanded {
    max-height: none;
  }

  .reasoning-content pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .message-content {
    line-height: 1.6;
  }

  .message-content pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .message-content :global(p) {
    margin: 0 0 var(--spacing-sm) 0;
  }

  .message-content :global(p:last-child) {
    margin-bottom: 0;
  }

  .message-content :global(code) {
    background: var(--color-bg-tertiary);
    padding: 2px var(--spacing-xs);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
  }

  .message-content :global(pre code) {
    background: transparent;
    padding: 0;
  }

  .text-muted {
    color: var(--color-text-muted);
  }
</style>
```

### 4. `frontend/src/lib/components/ChatView.svelte`

**Purpose:** Main chat view with message list and auto-scrolling.

```svelte
<script>
  import { currentChat, sortedMessages, chatLoading, chatError } from '../stores/chat.js';
  import Message from './Message.svelte';

  let messagesContainer = null;
  let isAtBottom = true;

  /**
   * Check if user is at the bottom of the message list.
   */
  function checkIfAtBottom() {
    if (!messagesContainer) return false;
    const threshold = 30;
    const { scrollTop, scrollHeight, clientHeight } = messagesContainer;
    return scrollHeight - scrollTop - clientHeight < threshold;
  }

  /**
   * Scroll to bottom of message list.
   */
  function scrollToBottom() {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }

  /**
   * Handle scroll event.
   */
  function handleScroll() {
    isAtBottom = checkIfAtBottom();
  }

  // Auto-scroll when messages change (only if at bottom)
  $: if ($sortedMessages && isAtBottom) {
    // Use setTimeout to ensure DOM is updated
    setTimeout(() => {
      if (isAtBottom) {
        scrollToBottom();
      }
    }, 0);
  }

  // Scroll to bottom when chat loads
  $: if ($sortedMessages && !$chatLoading) {
    setTimeout(() => scrollToBottom(), 0);
  }
</script>

<div class="chat-view">
  {#if $chatLoading}
    <div class="chat-loading">
      <span class="text-muted">Loading chat...</span>
    </div>
  {:else if $chatError}
    <div class="chat-error">
      <span class="text-error">{$chatError}</span>
    </div>
  {:else if !$currentChat}
    <div class="chat-empty">
      <span class="text-muted">No chat selected</span>
    </div>
  {:else}
    <div class="chat-header">
      <h2>{$currentChat.title}</h2>
      {#if $currentChat.tags && $currentChat.tags.length > 0}
        <div class="chat-tags">
          {#each $currentChat.tags as tag}
            <span class="tag">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>

    <div
      class="messages-container"
      bind:this={messagesContainer}
      on:scroll={handleScroll}
    >
      {#if $sortedMessages.length === 0}
        <div class="messages-empty">
          <span class="text-muted">No messages yet</span>
        </div>
      {:else}
        <div class="messages-list">
          {#each $sortedMessages as message (message.id)}
            <Message {message} />
          {/each}
        </div>
      {/if}
    </div>

    <div class="chat-input-placeholder">
      <p class="text-muted">Message input will be added in Phase 4</p>
    </div>
  {/if}
</div>

<style>
  .chat-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .chat-loading,
  .chat-error,
  .chat-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .chat-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chat-header h2 {
    margin: 0 0 var(--spacing-xs) 0;
    font-size: var(--font-size-lg);
  }

  .chat-tags {
    display: flex;
    gap: var(--spacing-xs);
  }

  .messages-container {
    flex: 1;
    overflow-y: auto;
  }

  .messages-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .messages-list {
    display: flex;
    flex-direction: column;
  }

  .chat-input-placeholder {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
```

### 5. `frontend/src/App.svelte`

**Modify:** Integrate ChatView component.

```svelte
<script>
  import { onMount } from 'svelte';
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';
  import { initChats } from './lib/stores/chats.js';
  import { selectChat, clearChat } from './lib/stores/chat.js';
  import TabView from './lib/components/TabView.svelte';
  import ChatList from './lib/components/ChatList.svelte';
  import ChatView from './lib/components/ChatView.svelte';

  /** @type {'chats' | 'plugins'} */
  let activeTab = 'chats';

  /** @type {number | null} */
  let selectedChatId = null;

  onMount(() => {
    websocket.connect();

    // Wait for connection, then initialize chats
    const unsubscribe = connectionStore.subscribe(({ status }) => {
      if (status === 'connected') {
        initChats();
        unsubscribe();
      }
    });
  });

  function handleTabChange(event) {
    activeTab = event.detail.tab;
  }

  async function handleChatSelect(event) {
    selectedChatId = event.detail.chatId;
    await selectChat(selectedChatId);
  }
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>

  <TabView {activeTab} on:tabChange={handleTabChange} />

  <main class="app-main">
    {#if activeTab === 'chats'}
      <div class="chats-layout">
        <aside class="chats-sidebar">
          <ChatList {selectedChatId} on:chatSelect={handleChatSelect} />
        </aside>
        <section class="chats-content">
          <ChatView />
        </section>
      </div>
    {:else if activeTab === 'plugins'}
      <div class="plugins-content">
        <p>Plugins tab will be rendered here (Phase 5)</p>
      </div>
    {/if}
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
  }

  .chats-layout {
    display: flex;
    height: 100%;
  }

  .chats-sidebar {
    width: 300px;
    flex-shrink: 0;
  }

  .chats-content {
    flex: 1;
    overflow: hidden;
  }

  .plugins-content {
    padding: var(--spacing-lg);
  }
</style>
```

## Tests

### Manual Testing

1. **Chat Selection:**
   - Click a chat in the list
   - Verify chat view loads with messages
   - Verify chat title and tags displayed in header
   - Verify messages displayed in chronological order

2. **Message Display:**
   - Verify messages show role badge, timestamp, and content
   - Verify Markdown rendering works (bold, italic, code blocks, etc.)
   - Verify tags displayed on messages
   - Verify messages with reasoning content show reasoning section

3. **Markdown Toggle:**
   - Click Markdown toggle button on a message
   - Verify content switches between Markdown and plain text
   - Verify toggle button icon changes

4. **Reasoning Content:**
   - Verify reasoning content shows last 3 lines when collapsed
   - Click expand button - verify full content shown
   - Click collapse button - verify back to 3 lines
   - Verify reasoning content scrolls to bottom when new content arrives (if collapsed)

5. **Auto-Scrolling:**
   - Send a message (will be added in Phase 4, but test with existing messages)
   - Verify auto-scroll to bottom when at bottom
   - Scroll up - verify auto-scroll stops
   - Scroll back to bottom - verify auto-scroll resumes

6. **Real-Time Updates:**
   - Add a message from another client
   - Verify message appears in chat view automatically
   - Update a message from another client
   - Verify message updates in chat view
   - Delete a message from another client
   - Verify message disappears from chat view

7. **Empty States:**
   - Select a chat with no messages - verify "No messages yet" shown
   - Deselect chat - verify "No chat selected" shown

8. **Loading States:**
   - Select a chat - verify loading indicator shown while loading
   - Verify loading indicator disappears after load

## Implementation Notes

1. **Markdown Rendering**: Uses `marked` library with `breaks: true` option to convert line breaks to `<br>` tags. HTML output should be sanitized in production (consider adding DOMPurify).

2. **Reasoning Content Display**: Uses CSS `max-height` with `overflow: hidden` to show ~3 lines. Content is scrolled to bottom within the container using JavaScript. When expanded, `max-height` is removed.

3. **Auto-Scrolling**: Tracks scroll position with 30px threshold. Only auto-scrolls if user is already at bottom. Uses `setTimeout` to ensure DOM is updated before scrolling.

4. **Message Sorting**: Messages sorted by `createdAt` ASC using derived store. Sorting happens reactively when messages change.

5. **Event Filtering**: Chat events are filtered by `chatId` to ensure only events for the selected chat are processed.

6. **Cleanup**: When selecting a new chat, the previous chat's event listeners are cleaned up and the subscription is cancelled.

7. **Queue Message Styling**: The `isQueue` prop is added to the Message component for Phase 4, but not used yet. It will enable `opacity: 0.8` styling for queue messages.

## Dependencies

- **Requires**: Phase 1 (WebSocket), Phase 2 (chat list for selection)
- **Blocks**: Phase 4 (needs message display for queue messages)

## Success Criteria

- [ ] Selecting a chat loads and displays its messages
- [ ] Messages render with Markdown formatting by default
- [ ] Toggle button switches between Markdown and plain text
- [ ] Reasoning content shows last 3 lines when collapsed
- [ ] Expand/collapse button works smoothly
- [ ] Auto-scroll works only when user is at bottom
- [ ] Messages update in real-time when added/updated/deleted
- [ ] Tags display correctly on messages
- [ ] Chat view clears when chat is deselected
- [ ] Loading state shown while fetching chat
- [ ] Error state shown when operations fail
- [ ] Empty state shown when no messages or no chat selected
- [ ] Chat header shows title and tags
- [ ] Messages sorted chronologically
