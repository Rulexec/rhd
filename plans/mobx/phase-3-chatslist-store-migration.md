# Phase 3: ChatsListStore Migration

## Overview

This phase migrates chat list management from Svelte stores to MobX store. The ChatsListStore will manage the list of chats, loading state, error state, and provide methods for chat list operations (init, load, create, delete).

## Files to Create

### 1. `frontend/src/stores/ChatsListStore.ts`

**Purpose**: MobX store for chat list management.

**Additions**:
```typescript
import { makeAutoObservable, flow } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat } from '../lib/api/schemas.js';

/**
 * MobX store for chat list management.
 */
export class ChatsListStore {
  #chatApi: ChatApi;
  #chats: Chat[] = [];
  #cleanupEvents: (() => void) | null = null;

  loading: boolean = false;
  error: string | null = null;

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Get chats sorted by updatedAt DESC.
   */
  get chats(): Chat[] {
    return [...this.#chats].sort((a, b) => {
      return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
    });
  }

  /**
   * Check if there are any chats.
   */
  get hasChats(): boolean {
    return this.#chats.length > 0;
  }

  /**
   * Initialize the store: subscribe to events and load initial data.
   * Should be called once on app startup.
   */
  *init(): Generator {
    this.error = null;

    try {
      // Subscribe to chat list events
      yield* yieldPromise(this.#chatApi.subscribeChatsList());

      // Register event listeners
      this.#cleanupEvents = this.#chatApi.onChatListEvents({
        onChatCreated: ({ chat }) => {
          this.#handleChatCreated(chat);
        },
        onChatUpdated: ({ chat }) => {
          this.#handleChatUpdated(chat);
        },
        onChatDeleted: ({ chatId }) => {
          this.#handleChatDeleted(chatId);
        }
      });

      // Load initial chat list
      yield* yieldPromise(this.loadChats());
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Load chats from server.
   */
  *loadChats(): Generator {
    this.loading = true;
    this.error = null;

    try {
      const result = yield* yieldPromise(this.#chatApi.listChats());
      this.#chats = result.chats;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Create a new chat with auto-generated title.
   * @returns The created chat ID, or null on error
   */
  *createNewChat(): Generator<unknown, number | null, unknown> {
    this.error = null;

    try {
      const title = this.#chatApi.generateChatTitle();
      const result = yield* yieldPromise(this.#chatApi.createChat(title));
      // Chat will be added via chatCreated event
      return result.chatId;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
      return null;
    }
  }

  /**
   * Delete all chats.
   * @returns True if successful
   */
  *deleteAllChats(): Generator<unknown, boolean, unknown> {
    this.error = null;

    try {
      if (!this.#chats || this.#chats.length === 0) {
        return true;
      }

      // Delete each chat
      for (const chat of this.#chats) {
        yield* yieldPromise(this.#chatApi.deleteChat(chat.id));
      }

      return true;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
      return false;
    }
  }

  /**
   * Clear the store state.
   */
  clear(): void {
    if (this.#cleanupEvents) {
      this.#cleanupEvents();
      this.#cleanupEvents = null;
    }
    this.#chats = [];
    this.loading = false;
    this.error = null;
  }

  /**
   * Handle chat created event.
   */
  #handleChatCreated(chat: Chat): void {
    // Don't add if already exists
    if (this.#chats.some(c => c.id === chat.id)) {
      this.#chats = this.#chats.map(c => c.id === chat.id ? chat : c);
    } else {
      this.#chats = [...this.#chats, chat];
    }
  }

  /**
   * Handle chat updated event.
   */
  #handleChatUpdated(chat: Chat): void {
    this.#chats = this.#chats.map(c => c.id === chat.id ? chat : c);
  }

  /**
   * Handle chat deleted event.
   */
  #handleChatDeleted(chatId: number): void {
    this.#chats = this.#chats.filter(c => c.id !== chatId);
  }
}
```

## Files to Modify

### 1. `frontend/src/stores/AppStore.ts`

**Modifications**: Add `chatsList` lazy getter.

**Changes**: Add the following to AppStore class:

```typescript
import { ChatsListStore } from './ChatsListStore.js';

export class AppStore {
  #chatApi: ChatApi;
  #chatsListStore?: ChatsListStore;
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

  // ... other getters
}
```

## Tests

### Unit Tests for ChatsListStore

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatsListStore } from './ChatsListStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat } from '../lib/api/schemas.js';

describe('ChatsListStore', () => {
  let store: ChatsListStore;
  let mockChatApi: ChatApi;

  const mockChat: Chat = {
    id: 1,
    title: 'Test Chat',
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    tags: [],
    version: 1
  };

  beforeEach(() => {
    mockChatApi = {
      subscribeChatsList: vi.fn().mockResolvedValue(undefined),
      unsubscribeChatsList: vi.fn().mockResolvedValue(undefined),
      listChats: vi.fn().mockResolvedValue({ chats: [mockChat] }),
      createChat: vi.fn().mockResolvedValue({ chatId: 2 }),
      deleteChat: vi.fn().mockResolvedValue(undefined),
      generateChatTitle: vi.fn().mockReturnValue('2024-01-01 00:00'),
      onChatListEvents: vi.fn().mockReturnValue(() => {}),
      // ... other methods
    } as unknown as ChatApi;

    store = new ChatsListStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.chats).toEqual([]);
    expect(store.hasChats).toBe(false);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should load chats', async () => {
    await store.loadChats();
    
    expect(mockChatApi.listChats).toHaveBeenCalled();
    expect(store.chats).toEqual([mockChat]);
    expect(store.hasChats).toBe(true);
    expect(store.loading).toBe(false);
  });

  it('should handle load error', async () => {
    vi.mocked(mockChatApi.listChats).mockRejectedValueOnce(new Error('Load failed'));
    
    await store.loadChats();
    
    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should create new chat', async () => {
    const chatId = await store.createNewChat();
    
    expect(mockChatApi.generateChatTitle).toHaveBeenCalled();
    expect(mockChatApi.createChat).toHaveBeenCalled();
    expect(chatId).toBe(2);
  });

  it('should delete all chats', async () => {
    await store.loadChats();
    const success = await store.deleteAllChats();
    
    expect(mockChatApi.deleteChat).toHaveBeenCalledWith(1);
    expect(success).toBe(true);
  });

  it('should sort chats by updatedAt DESC', async () => {
    const chat1: Chat = { ...mockChat, id: 1, updatedAt: '2024-01-01T00:00:00Z' };
    const chat2: Chat = { ...mockChat, id: 2, updatedAt: '2024-01-02T00:00:00Z' };
    
    vi.mocked(mockChatApi.listChats).mockResolvedValueOnce({ chats: [chat1, chat2] });
    await store.loadChats();
    
    expect(store.chats[0].id).toBe(2); // Newer first
    expect(store.chats[1].id).toBe(1);
  });

  it('should handle chat created event', async () => {
    let onChatCreatedHandler: ((data: { chat: Chat }) => void) | undefined;
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatCreatedHandler = handlers.onChatCreated;
      return () => {};
    });

    await store.init();
    
    const newChat: Chat = { ...mockChat, id: 3 };
    onChatCreatedHandler!({ chat: newChat });
    
    expect(store.chats.some(c => c.id === 3)).toBe(true);
  });

  it('should handle chat updated event', async () => {
    let onChatUpdatedHandler: ((data: { chat: Chat }) => void) | undefined;
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatUpdatedHandler = handlers.onChatUpdated;
      return () => {};
    });

    await store.loadChats();
    
    const updatedChat: Chat = { ...mockChat, title: 'Updated Title' };
    onChatUpdatedHandler!({ chat: updatedChat });
    
    expect(store.chats[0].title).toBe('Updated Title');
  });

  it('should handle chat deleted event', async () => {
    let onChatDeletedHandler: ((data: { chatId: number }) => void) | undefined;
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatDeletedHandler = handlers.onChatDeleted;
      return () => {};
    });

    await store.loadChats();
    expect(store.chats.length).toBe(1);
    
    onChatDeletedHandler!({ chatId: 1 });
    
    expect(store.chats.length).toBe(0);
  });

  it('should cleanup on clear', async () => {
    const cleanup = vi.fn();
    vi.mocked(mockChatApi.onChatListEvents).mockReturnValue(cleanup);

    await store.init();
    store.clear();
    
    expect(cleanup).toHaveBeenCalled();
    expect(store.chats).toEqual([]);
  });
});
```

## Implementation Notes

1. **Generator Functions**: All async methods use generator syntax (`*methodName()`) with `yield* yieldPromise()` for proper typing with MobX flow.

2. **Private Fields**: 
   - `#chats` stores the raw chat list (unsorted)
   - `#chatApi` holds the API reference
   - `#cleanupEvents` holds the event listener cleanup function

3. **Computed Getters**: 
   - `chats` returns sorted list (computed on access)
   - `hasChats` derives from `#chats.length`

4. **Event Handling**: Private methods `#handleChatCreated`, `#handleChatUpdated`, `#handleChatDeleted` handle WebSocket events and update state.

5. **Error Handling**: All async methods catch errors and store them in `error` property. Methods return appropriate values (null, false) on error.

6. **Immutability**: Array updates create new arrays (`[...this.#chats, chat]`, `this.#chats.map(...)`, `this.#chats.filter(...)`) to ensure MobX detects changes.

7. **Cleanup**: The `clear()` method calls the event cleanup function and resets state.

## Dependencies

- Depends on Phase 1 for infrastructure (MobX, utilities, ChatApi interface).
- Must be completed before Phase 6 (Component Migration).
- Can be done in parallel with Phases 2, 4, 5.
