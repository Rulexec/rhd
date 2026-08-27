# Phase 4: ChatStore Migration

## Overview

This phase migrates current chat state and message handling from Svelte stores to MobX store. The ChatStore will manage the current chat, messages, queue messages, loading state, and error state. It will handle chat selection, subscription to chat events, and message operations.

## Files to Create

### 1. `frontend/src/stores/ChatStore.ts`

**Purpose**: MobX store for current chat state and message management.

**Additions**:
```typescript
import { makeAutoObservable, flow } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat, Message } from '../lib/api/schemas.js';

/**
 * Message with queue flag for display.
 */
export interface DisplayMessage extends Message {
  isQueue: boolean;
}

/**
 * MobX store for current chat state.
 */
export class ChatStore {
  #chatApi: ChatApi;
  #cleanupEvents: (() => void) | null = null;
  #cleanupQueueEvents: (() => void) | null = null;

  currentChatId: number | null = null;
  currentChat: Chat | null = null;
  messages: Message[] = [];
  queueMessages: Message[] = [];
  loading: boolean = false;
  error: string | null = null;

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Get all messages (regular + queue) sorted by createdAt ASC.
   * Queue messages are marked with isQueue flag.
   */
  get allMessages(): DisplayMessage[] {
    const regular: DisplayMessage[] = this.messages.map(m => ({ ...m, isQueue: false }));
    const queue: DisplayMessage[] = this.queueMessages.map(m => ({ ...m, isQueue: true }));

    return [...regular, ...queue].sort((a, b) => {
      return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
    });
  }

  /**
   * Select a chat and load its messages.
   * @param chatId - Chat ID to select
   */
  *selectChat(chatId: number): Generator {
    // Cleanup previous subscription
    yield* yieldPromise(this.clearChat());

    this.currentChatId = chatId;
    this.loading = true;
    this.error = null;

    try {
      // Subscribe to chat events
      yield* yieldPromise(this.#chatApi.subscribeChat(chatId));

      // Register event listeners for regular messages
      this.#cleanupEvents = this.#chatApi.onChatEvents(chatId, {
        onMessageAdded: ({ message }) => {
          this.#handleMessageAdded(message);
        },
        onMessageUpdated: ({ message }) => {
          this.#handleMessageUpdated(message);
        },
        onMessageDeleted: ({ messageId }) => {
          this.#handleMessageDeleted(messageId);
        }
      });

      // Register event listeners for queue messages
      this.#cleanupQueueEvents = this.#chatApi.onQueueMessageEvents(chatId, {
        onQueueMessageAdded: ({ message }) => {
          this.#handleQueueMessageAdded(message);
        },
        onQueueMessageUpdated: ({ message }) => {
          this.#handleQueueMessageUpdated(message);
        },
        onQueueMessageDeleted: ({ messageId }) => {
          this.#handleQueueMessageDeleted(messageId);
        }
      });

      // Load chat data
      const result = yield* yieldPromise(this.#chatApi.getChat(chatId));
      this.currentChat = result.chat;
      this.messages = result.messages;

      // Load queue messages
      const queueResult = yield* yieldPromise(this.#chatApi.getQueueMessages(chatId));
      this.queueMessages = queueResult.messages;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Clear the current chat and unsubscribe.
   */
  *clearChat(): Generator {
    // Cleanup event listeners
    if (this.#cleanupEvents) {
      this.#cleanupEvents();
      this.#cleanupEvents = null;
    }

    // Cleanup queue event listeners
    if (this.#cleanupQueueEvents) {
      this.#cleanupQueueEvents();
      this.#cleanupQueueEvents = null;
    }

    // Unsubscribe from previous chat
    const prevChatId = this.currentChatId;
    if (prevChatId) {
      try {
        yield* yieldPromise(this.#chatApi.unsubscribeChat(prevChatId));
      } catch (error) {
        console.warn('Failed to unsubscribe from chat:', error);
      }
    }

    this.currentChatId = null;
    this.currentChat = null;
    this.messages = [];
    this.queueMessages = [];
    this.loading = false;
    this.error = null;
  }

  /**
   * Handle message added event.
   */
  #handleMessageAdded(message: Message): void {
    // Don't add if already exists
    if (this.messages.some(m => m.id === message.id)) {
      this.messages = this.messages.map(m => m.id === message.id ? message : m);
    } else {
      this.messages = [...this.messages, message];
    }

    // If this message was in the queue, remove it from queue
    this.queueMessages = this.queueMessages.filter(m => m.id !== message.id);
  }

  /**
   * Handle message updated event.
   */
  #handleMessageUpdated(message: Message): void {
    this.messages = this.messages.map(m => m.id === message.id ? message : m);
  }

  /**
   * Handle message deleted event.
   */
  #handleMessageDeleted(messageId: number): void {
    this.messages = this.messages.filter(m => m.id !== messageId);
  }

  /**
   * Handle queue message added event.
   */
  #handleQueueMessageAdded(message: Message): void {
    // Don't add if already exists
    if (this.queueMessages.some(m => m.id === message.id)) {
      this.queueMessages = this.queueMessages.map(m => m.id === message.id ? message : m);
    } else {
      this.queueMessages = [...this.queueMessages, message];
    }
  }

  /**
   * Handle queue message updated event.
   */
  #handleQueueMessageUpdated(message: Message): void {
    this.queueMessages = this.queueMessages.map(m => m.id === message.id ? message : m);
  }

  /**
   * Handle queue message deleted event.
   */
  #handleQueueMessageDeleted(messageId: number): void {
    this.queueMessages = this.queueMessages.filter(m => m.id !== messageId);
  }
}
```

## Files to Modify

### 1. `frontend/src/stores/AppStore.ts`

**Modifications**: Add `chat` lazy getter.

**Changes**: Add the following to AppStore class:

```typescript
import { ChatStore } from './ChatStore.js';

export class AppStore {
  #chatApi: ChatApi;
  #chatsListStore?: ChatsListStore;
  #chatStore?: ChatStore;
  // ... other fields

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Chat list store (lazy initialized).
   */
  get chatsList(): ChatsListStore {
    if (!this.#chatsListStore) {
      this.#chatsListStore = new ChatsListStore({ chatApi: this.#chatApi });
    }
    return this.#chatsListStore;
  }

  /**
   * Current chat store (lazy initialized).
   */
  get chat(): ChatStore {
    if (!this.#chatStore) {
      this.#chatStore = new ChatStore({ chatApi: this.#chatApi });
    }
    return this.#chatStore;
  }

  // ... other getters
}
```

## Tests

### Unit Tests for ChatStore

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatStore } from './ChatStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat, Message } from '../lib/api/schemas.js';

describe('ChatStore', () => {
  let store: ChatStore;
  let mockChatApi: ChatApi;

  const mockChat: Chat = {
    id: 1,
    title: 'Test Chat',
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    tags: [],
    version: 1
  };

  const mockMessage: Message = {
    id: 1,
    chatId: 1,
    role: 'user',
    content: 'Hello',
    createdAt: '2024-01-01T00:00:00Z',
    tags: [],
    isFinished: true,
    isStreaming: false
  };

  beforeEach(() => {
    mockChatApi = {
      subscribeChat: vi.fn().mockResolvedValue(undefined),
      unsubscribeChat: vi.fn().mockResolvedValue(undefined),
      getChat: vi.fn().mockResolvedValue({ chat: mockChat, messages: [mockMessage] }),
      onChatEvents: vi.fn().mockReturnValue(() => {}),
      getQueueMessages: vi.fn().mockResolvedValue({ messages: [] }),
      onQueueMessageEvents: vi.fn().mockReturnValue(() => {}),
      // ... other methods
    } as unknown as ChatApi;

    store = new ChatStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.currentChatId).toBe(null);
    expect(store.currentChat).toBe(null);
    expect(store.messages).toEqual([]);
    expect(store.queueMessages).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should select chat and load messages', async () => {
    await store.selectChat(1);
    
    expect(mockChatApi.subscribeChat).toHaveBeenCalledWith(1);
    expect(mockChatApi.getChat).toHaveBeenCalledWith(1);
    expect(mockChatApi.getQueueMessages).toHaveBeenCalledWith(1);
    expect(store.currentChatId).toBe(1);
    expect(store.currentChat).toEqual(mockChat);
    expect(store.messages).toEqual([mockMessage]);
    expect(store.loading).toBe(false);
  });

  it('should handle select error', async () => {
    vi.mocked(mockChatApi.getChat).mockRejectedValueOnce(new Error('Load failed'));
    
    await store.selectChat(1);
    
    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should clear chat', async () => {
    await store.selectChat(1);
    await store.clearChat();
    
    expect(mockChatApi.unsubscribeChat).toHaveBeenCalledWith(1);
    expect(store.currentChatId).toBe(null);
    expect(store.currentChat).toBe(null);
    expect(store.messages).toEqual([]);
    expect(store.queueMessages).toEqual([]);
  });

  it('should combine regular and queue messages in allMessages', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2, content: 'Queue msg' };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });
    
    await store.selectChat(1);
    
    const allMessages = store.allMessages;
    expect(allMessages.length).toBe(2);
    expect(allMessages[0].isQueue).toBe(false);
    expect(allMessages[1].isQueue).toBe(true);
  });

  it('should sort allMessages by createdAt ASC', async () => {
    const msg1: Message = { ...mockMessage, id: 1, createdAt: '2024-01-01T00:00:00Z' };
    const msg2: Message = { ...mockMessage, id: 2, createdAt: '2024-01-01T00:01:00Z' };
    vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({ chat: mockChat, messages: [msg2, msg1] });
    
    await store.selectChat(1);
    
    expect(store.allMessages[0].id).toBe(1); // Older first
    expect(store.allMessages[1].id).toBe(2);
  });

  it('should handle message added event', async () => {
    let onMessageAddedHandler: ((data: { message: Message }) => void) | undefined;
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((chatId, handlers) => {
      onMessageAddedHandler = handlers.onMessageAdded;
      return () => {};
    });

    await store.selectChat(1);
    
    const newMessage: Message = { ...mockMessage, id: 3 };
    onMessageAddedHandler!({ message: newMessage });
    
    expect(store.messages.some(m => m.id === 3)).toBe(true);
  });

  it('should remove message from queue when added as regular message', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2 };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });
    
    let onMessageAddedHandler: ((data: { message: Message }) => void) | undefined;
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((chatId, handlers) => {
      onMessageAddedHandler = handlers.onMessageAdded;
      return () => {};
    });

    await store.selectChat(1);
    expect(store.queueMessages.length).toBe(1);
    
    // Message added as regular message
    onMessageAddedHandler!({ message: queueMessage });
    
    expect(store.queueMessages.length).toBe(0);
    expect(store.messages.some(m => m.id === 2)).toBe(true);
  });

  it('should handle message updated event', async () => {
    let onMessageUpdatedHandler: ((data: { message: Message }) => void) | undefined;
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((chatId, handlers) => {
      onMessageUpdatedHandler = handlers.onMessageUpdated;
      return () => {};
    });

    await store.selectChat(1);
    
    const updatedMessage: Message = { ...mockMessage, content: 'Updated' };
    onMessageUpdatedHandler!({ message: updatedMessage });
    
    expect(store.messages[0].content).toBe('Updated');
  });

  it('should handle message deleted event', async () => {
    let onMessageDeletedHandler: ((data: { messageId: number }) => void) | undefined;
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((chatId, handlers) => {
      onMessageDeletedHandler = handlers.onMessageDeleted;
      return () => {};
    });

    await store.selectChat(1);
    expect(store.messages.length).toBe(1);
    
    onMessageDeletedHandler!({ messageId: 1 });
    
    expect(store.messages.length).toBe(0);
  });

  it('should handle queue message added event', async () => {
    let onQueueMessageAddedHandler: ((data: { message: Message }) => void) | undefined;
    vi.mocked(mockChatApi.onQueueMessageEvents).mockImplementation((chatId, handlers) => {
      onQueueMessageAddedHandler = handlers.onQueueMessageAdded;
      return () => {};
    });

    await store.selectChat(1);
    
    const queueMessage: Message = { ...mockMessage, id: 3 };
    onQueueMessageAddedHandler!({ message: queueMessage });
    
    expect(store.queueMessages.some(m => m.id === 3)).toBe(true);
  });

  it('should cleanup event listeners on clear', async () => {
    const cleanupEvents = vi.fn();
    const cleanupQueueEvents = vi.fn();
    
    vi.mocked(mockChatApi.onChatEvents).mockReturnValue(cleanupEvents);
    vi.mocked(mockChatApi.onQueueMessageEvents).mockReturnValue(cleanupQueueEvents);

    await store.selectChat(1);
    await store.clearChat();
    
    expect(cleanupEvents).toHaveBeenCalled();
    expect(cleanupQueueEvents).toHaveBeenCalled();
  });
});
```

## Implementation Notes

1. **Generator Functions**: All async methods use generator syntax (`*methodName()`) with `yield* yieldPromise()` for proper typing with MobX flow.

2. **Private Fields**: 
   - `#chatApi` holds the API reference
   - `#cleanupEvents` holds the chat event listener cleanup function
   - `#cleanupQueueEvents` holds the queue event listener cleanup function

3. **Computed Getter**: `allMessages` combines regular and queue messages, marks them with `isQueue` flag, and sorts by `createdAt` ASC.

4. **Event Handling**: Private methods handle WebSocket events and update state. When a message is added as a regular message, it's automatically removed from the queue.

5. **Error Handling**: All async methods catch errors and store them in `error` property.

6. **Immutability**: Array updates create new arrays to ensure MobX detects changes.

7. **Cleanup**: The `clearChat()` method calls both event cleanup functions and unsubscribes from the chat.

8. **Subscription Lifecycle**: `selectChat()` first clears any previous subscription before setting up a new one.

## Dependencies

- Depends on Phase 1 for infrastructure (MobX, utilities, ChatApi interface).
- Must be completed before Phase 6 (Component Migration).
- Can be done in parallel with Phases 2, 3, 5.
