# Phase 4: Queue Messages & Input

## Overview

Implement the message input component and queue message handling. This phase enables users to send messages to the chat, which are added to the queue and displayed with distinct styling.

**Scope:**
- Message input component with multi-line textarea and auto-resize
- Queue message API methods
- Queue message store integration
- Queue message display with distinct styling (opacity 0.8)
- Input validation and error handling

**Out of Scope:**
- Connection status UI (Phase 6)
- Plugins tab (Phase 5)
- Advanced error handling polish (Phase 6)

## Dependencies

- **Requires**: Phase 3 (message display infrastructure)
- **Blocks**: Phase 6 (needs queue handling for polish)

## Files to Create/Modify

### 1. `frontend/src/lib/api/chatApi.js`

**Modify:** Add methods for queue message operations.

```javascript
// Add these imports at the top
import {
  // ... existing imports ...
  GetQueueMessagesResultSchema,
  QueueMessageAddedDataSchema,
  QueueMessageUpdatedDataSchema,
  QueueMessageDeletedDataSchema
} from './schemas.js';

/**
 * Get queue messages for a chat.
 * @param {number} chatId - Chat ID
 * @returns {Promise<import('./schemas.js').GetQueueMessagesResult>}
 */
export async function getQueueMessages(chatId) {
  const data = await websocket.request('getQueueMessages', { chatId });
  return GetQueueMessagesResultSchema.parse(data);
}

/**
 * Add a message to the queue.
 * @param {number} chatId - Chat ID
 * @param {string} role - Message role (e.g., "user")
 * @param {string} content - Message content
 * @param {string[]} [tags] - Optional tags
 * @returns {Promise<void>}
 */
export async function addQueueMessage(chatId, role, content, tags = []) {
  await websocket.request('addQueueMessage', {
    chatId,
    role,
    content,
    tags
  });
}

/**
 * Register event listeners for queue message events.
 * @param {number} chatId - Chat ID to listen for
 * @param {Object} handlers
 * @param {Function} handlers.onQueueMessageAdded
 * @param {Function} handlers.onQueueMessageUpdated
 * @param {Function} handlers.onQueueMessageDeleted
 * @returns {Function} - Cleanup function
 */
export function onQueueMessageEvents(chatId, { onQueueMessageAdded, onQueueMessageUpdated, onQueueMessageDeleted }) {
  const unsubs = [];

  if (onQueueMessageAdded) {
    unsubs.push(websocket.on('queueMessageAdded', (data) => {
      const parsed = QueueMessageAddedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onQueueMessageAdded(parsed);
      }
    }));
  }

  if (onQueueMessageUpdated) {
    unsubs.push(websocket.on('queueMessageUpdated', (data) => {
      const parsed = QueueMessageUpdatedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onQueueMessageUpdated(parsed);
      }
    }));
  }

  if (onQueueMessageDeleted) {
    unsubs.push(websocket.on('queueMessageDeleted', (data) => {
      const parsed = QueueMessageDeletedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        onQueueMessageDeleted(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
```

### 2. `frontend/src/lib/stores/chat.js`

**Modify:** Add queue message handling and merge queue with regular messages.

```javascript
// Add these imports at the top
import { getQueueMessages, onQueueMessageEvents } from '../api/chatApi.js';

/**
 * Queue messages for the current chat.
 * @type {import('svelte/store').Writable<import('../api/schemas.js').Message[]>}
 */
export const queueMessages = writable([]);

/**
 * Cleanup function for queue event listeners.
 * @type {Function | null}
 */
let cleanupQueueEvents = null;

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

    // Register event listeners for regular messages
    cleanupEvents = onChatEvents(chatId, {
      onMessageAdded: ({ message }) => {
        messages.update(msgs => {
          // Don't add if already exists
          if (msgs.some(m => m.id === message.id)) {
            return msgs.map(m => m.id === message.id ? message : m);
          }
          return [...msgs, message];
        });

        // If this message was in the queue, remove it from queue
        queueMessages.update(qMsgs => qMsgs.filter(m => m.id !== message.id));
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

    // Register event listeners for queue messages
    cleanupQueueEvents = onQueueMessageEvents(chatId, {
      onQueueMessageAdded: ({ message }) => {
        queueMessages.update(qMsgs => {
          // Don't add if already exists
          if (qMsgs.some(m => m.id === message.id)) {
            return qMsgs.map(m => m.id === message.id ? message : m);
          }
          return [...qMsgs, message];
        });
      },
      onQueueMessageUpdated: ({ message }) => {
        queueMessages.update(qMsgs =>
          qMsgs.map(m => m.id === message.id ? message : m)
        );
      },
      onQueueMessageDeleted: ({ messageId }) => {
        queueMessages.update(qMsgs =>
          qMsgs.filter(m => m.id !== messageId)
        );
      }
    });

    // Load chat data
    const result = await getChat(chatId);
    currentChat.set(result.chat);
    messages.set(result.messages);

    // Load queue messages
    const queueResult = await getQueueMessages(chatId);
    queueMessages.set(queueResult.messages);
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

  // Cleanup queue event listeners
  if (cleanupQueueEvents) {
    cleanupQueueEvents();
    cleanupQueueEvents = null;
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
  queueMessages.set([]);
  chatLoading.set(false);
  chatError.set(null);
}

/**
 * Derived store: all messages (regular + queue) sorted by createdAt ASC.
 * Queue messages are marked with isQueue flag.
 */
export const allMessages = derived([messages, queueMessages], ([$messages, $queueMessages]) => {
  const regular = $messages.map(m => ({ ...m, isQueue: false }));
  const queue = $queueMessages.map(m => ({ ...m, isQueue: true }));
  
  return [...regular, ...queue].sort((a, b) => {
    return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
  });
});
```

### 3. `frontend/src/lib/components/MessageInput.svelte`

**Purpose:** Multi-line textarea with send button and auto-resize.

```svelte
<script>
  import { createEventDispatcher } from 'svelte';
  import { currentChatId } from '../stores/chat.js';
  import { connectionStore } from '../stores/connection.js';
  import { addQueueMessage } from '../api/chatApi.js';

  const dispatch = createEventDispatcher();

  let inputValue = '';
  let textareaEl = null;
  let isSending = false;
  let errorMessage = null;

  $: isConnected = $connectionStore.status === 'connected';
  $: canSend = inputValue.trim().length > 0 && $currentChatId && isConnected && !isSending;

  /**
   * Auto-resize textarea based on content.
   */
  function autoResize() {
    if (!textareaEl) return;
    
    // Reset height to auto to get correct scrollHeight
    textareaEl.style.height = 'auto';
    
    // Set height to scrollHeight (max 200px)
    const newHeight = Math.min(textareaEl.scrollHeight, 200);
    textareaEl.style.height = `${newHeight}px`;
  }

  /**
   * Handle input event.
   */
  function handleInput() {
    autoResize();
    errorMessage = null;
  }

  /**
   * Handle keydown event.
   */
  function handleKeydown(event) {
    // Enter without Shift sends message
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleSend();
    }
    // Shift+Enter adds new line (default behavior)
  }

  /**
   * Send message to queue.
   */
  async function handleSend() {
    if (!canSend) return;

    const content = inputValue.trim();
    if (!content) return;

    isSending = true;
    errorMessage = null;

    try {
      await addQueueMessage($currentChatId, 'user', content);
      inputValue = '';
      
      // Reset textarea height
      if (textareaEl) {
        textareaEl.style.height = 'auto';
      }
      
      dispatch('messageSent');
    } catch (error) {
      errorMessage = error.message || 'Failed to send message';
    } finally {
      isSending = false;
    }
  }
</script>

<div class="message-input">
  {#if errorMessage}
    <div class="input-error">
      <span class="text-error">{errorMessage}</span>
    </div>
  {/if}

  <div class="input-container">
    <textarea
      bind:this={textareaEl}
      bind:value={inputValue}
      on:input={handleInput}
      on:keydown={handleKeydown}
      placeholder="Type a message... (Enter to send, Shift+Enter for new line)"
      disabled={!isConnected || !$currentChatId}
      rows="1"
      class="input-textarea"
    ></textarea>
    
    <button
      class="send-button"
      class:active={canSend}
      disabled={!canSend}
      on:click={handleSend}
      title="Send message"
    >
      {isSending ? '⏳' : '📤'}
    </button>
  </div>

  {#if !$currentChatId}
    <div class="input-hint">
      <span class="text-muted">Select a chat to send messages</span>
    </div>
  {:else if !isConnected}
    <div class="input-hint">
      <span class="text-error">Not connected to server</span>
    </div>
  {/if}
</div>

<style>
  .message-input {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .input-error {
    padding: var(--spacing-xs) var(--spacing-sm);
    margin-bottom: var(--spacing-sm);
    background: var(--color-error-bg);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
  }

  .input-container {
    display: flex;
    gap: var(--spacing-sm);
    align-items: flex-end;
  }

  .input-textarea {
    flex: 1;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg);
    color: var(--color-text);
    font-size: var(--font-size-md);
    font-family: inherit;
    line-height: 1.5;
    resize: none;
    overflow: hidden;
    min-height: 40px;
    max-height: 200px;
  }

  .input-textarea:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .input-textarea:disabled {
    background: var(--color-bg-tertiary);
    cursor: not-allowed;
  }

  .send-button {
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    border-radius: var(--radius-md);
    background: var(--color-bg-tertiary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-lg);
    cursor: not-allowed;
    transition: all var(--transition-fast);
  }

  .send-button.active {
    background: var(--color-primary);
    color: white;
    cursor: pointer;
  }

  .send-button.active:hover {
    background: var(--color-primary-hover);
  }

  .send-button:disabled {
    opacity: 0.5;
  }

  .input-hint {
    margin-top: var(--spacing-xs);
    font-size: var(--font-size-xs);
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
```

### 4. `frontend/src/lib/components/ChatView.svelte`

**Modify:** Integrate MessageInput and display queue messages with distinct styling.

```svelte
<script>
  import { currentChat, allMessages, chatLoading, chatError } from '../stores/chat.js';
  import Message from './Message.svelte';
  import MessageInput from './MessageInput.svelte';

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

  /**
   * Handle message sent.
   */
  function handleMessageSent() {
    // Scroll to bottom after sending
    setTimeout(() => scrollToBottom(), 0);
  }

  // Auto-scroll when messages change (only if at bottom)
  $: if ($allMessages && isAtBottom) {
    // Use setTimeout to ensure DOM is updated
    setTimeout(() => {
      if (isAtBottom) {
        scrollToBottom();
      }
    }, 0);
  }

  // Scroll to bottom when chat loads
  $: if ($allMessages && !$chatLoading) {
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
      {#if $allMessages.length === 0}
        <div class="messages-empty">
          <span class="text-muted">No messages yet</span>
        </div>
      {:else}
        <div class="messages-list">
          {#each $allMessages as message (message.id)}
            <Message {message} isQueue={message.isQueue} />
          {/each}
        </div>
      {/if}
    </div>

    <MessageInput on:messageSent={handleMessageSent} />
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

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
```

## Tests

### Manual Testing

1. **Message Input:**
   - Click on textarea - verify it's focused
   - Type a message - verify textarea auto-resizes
   - Type multiple lines - verify textarea grows (max 200px)
   - Press Enter - verify message is sent
   - Press Shift+Enter - verify new line is added
   - Click Send button - verify message is sent

2. **Queue Message Display:**
   - Send a message - verify it appears in message list
   - Verify queue message has opacity 0.8
   - Verify queue message appears at the end of the list
   - Verify queue message shows role, content, timestamp

3. **Queue Message Processing:**
   - Wait for plugin to process queue message (if AI completions plugin is running)
   - Verify queue message becomes regular message (opacity returns to 1.0)
   - Verify message content is preserved

4. **Input Validation:**
   - Try to send empty message - verify Send button is disabled
   - Try to send whitespace-only message - verify it's trimmed and not sent
   - Disconnect WebSocket - verify Send button is disabled
   - Deselect chat - verify Send button is disabled

5. **Error Handling:**
   - Send message while disconnected - verify error shown inline
   - Send message with network error - verify error shown inline
   - Verify error message is dismissible (clears on next input)

6. **Auto-Scroll:**
   - Send a message - verify auto-scroll to bottom
   - Scroll up, then send a message - verify no auto-scroll
   - Scroll back to bottom - verify auto-scroll resumes

7. **Real-Time Updates:**
   - Add queue message from another client - verify it appears
   - Update queue message from another client - verify it updates
   - Delete queue message from another client - verify it disappears

8. **Textarea Behavior:**
   - Type long message - verify textarea grows
   - Delete content - verify textarea shrinks
   - Clear message after sending - verify textarea resets height

## Implementation Notes

1. **Queue Message Flow**: User sends message → `addQueueMessage` API call → server emits `queueMessageAdded` event → store updates → UI shows queue message with opacity 0.8 → plugin processes → server emits `messageAdded` event → store removes from queue and adds to regular messages → UI updates.

2. **Queue Message Identification**: Queue messages are identified by their presence in the `queueMessages` store. When merged with regular messages in `allMessages`, they're marked with `isQueue: true` flag.

3. **Auto-Resize**: Textarea uses JavaScript to dynamically adjust height based on content. Height is reset to `auto` before measuring `scrollHeight` to get accurate measurement.

4. **Enter vs Shift+Enter**: Enter key sends message (prevents default to avoid new line). Shift+Enter adds new line (default textarea behavior).

5. **Error Handling**: Errors are shown inline in the MessageInput component. Error message clears when user starts typing again.

6. **Disabled States**: Send button is disabled when:
   - Input is empty or whitespace-only
   - No chat is selected
   - WebSocket is disconnected
   - Message is currently being sent

7. **Message Deduplication**: When a queue message is processed and becomes a regular message, the store checks if the message ID already exists in the regular messages list to avoid duplicates.

## Dependencies

- **Requires**: Phase 3 (message display infrastructure)
- **Blocks**: Phase 6 (needs queue handling for polish)

## Success Criteria

- [ ] Message input accepts multi-line text with auto-resize
- [ ] Enter sends message, Shift+Enter adds new line
- [ ] Sent messages appear in queue with opacity 0.8
- [ ] Queue messages update in real-time
- [ ] When queue message is processed, it becomes regular message
- [ ] Input clears after sending
- [ ] Send button disabled when input is empty or WebSocket disconnected
- [ ] Error shown inline if message send fails
- [ ] Textarea auto-resizes correctly (min 40px, max 200px)
- [ ] Queue messages sorted chronologically with regular messages
- [ ] Auto-scroll works after sending message
- [ ] Input disabled when no chat selected
- [ ] Input disabled when WebSocket disconnected
