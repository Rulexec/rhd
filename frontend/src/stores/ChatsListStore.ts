import { makeAutoObservable, flowResult } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat } from '../lib/api/schemas.js';

/**
 * MobX store for chat list management.
 *
 * Async operations are written as generator PROTOTYPE methods. `makeAutoObservable`
 * auto-wraps generator methods into flows (actions wrapping a promise chain), so
 * calling e.g. `store.loadChats()` returns a CancellablePromise, not a suspended
 * generator. Callers that want an explicitly-typed promise use `flowResult(...)`.
 *
 * State fields that drive reactive getters MUST be public (or underscore-prefixed)
 * observable fields: `#`-private class fields are NOT observable by MobX, so
 * `_chats` is stored publicly while non-reactive dependencies (`#chatApi`,
 * `#cleanupEvents`) may stay private.
 */
export class ChatsListStore {
  #chatApi: ChatApi;
  #cleanupEvents: (() => void) | null = null;

  /** Raw (unsorted) chat list. Public so MobX can observe it. */
  _chats: Chat[] = [];

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
    return [...this._chats].sort((a, b) => {
      return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
    });
  }

  /**
   * Check if there are any chats.
   */
  get hasChats(): boolean {
    return this._chats.length > 0;
  }

  /**
   * Get all unique tags across all chats (for suggestions).
   */
  get allTags(): string[] {
    const tagSet = new Set<string>();
    for (const chat of this._chats) {
      for (const tag of chat.tags) {
        tagSet.add(tag);
      }
    }
    return Array.from(tagSet).sort();
  }

  /**
   * Initialize the store: subscribe to events and load initial data.
   * Should be called once on app startup.
   */
  *init(): Generator<unknown, void, unknown> {
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
      yield* yieldPromise(flowResult(this.loadChats()));
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Load chats from server.
   */
  *loadChats(): Generator<unknown, void, unknown> {
    this.loading = true;
    this.error = null;

    try {
      const result = yield* yieldPromise(this.#chatApi.listChats());
      this._chats = result.chats;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Create a new chat with auto-generated title.
   * The new chat is added to the list via the chatCreated event.
   * Note: selecting the created chat is the responsibility of the caller
   * (e.g. ChatList component), not this store.
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
      if (!this._chats || this._chats.length === 0) {
        return true;
      }

      // Delete each chat
      for (const chat of this._chats) {
        yield* yieldPromise(this.#chatApi.deleteChat(chat.id));
      }

      return true;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
      return false;
    }
  }

  /**
   * Add tags to a chat.
   * @param chatId - Chat ID
   * @param tags - Tags to add
   */
  *addChatTags(chatId: number, tags: string[]): Generator<unknown, void, unknown> {
    this.error = null;
    try {
      yield* yieldPromise(this.#chatApi.updateChat({
        chatId,
        addTags: tags
      }));
      // Chat will be updated via chatUpdated event
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Clear the store state and unsubscribe from events.
   */
  clear(): void {
    if (this.#cleanupEvents) {
      this.#cleanupEvents();
      this.#cleanupEvents = null;
    }
    this._chats = [];
    this.loading = false;
    this.error = null;
  }

  /**
   * Handle chat created event.
   */
  #handleChatCreated(chat: Chat): void {
    // Don't add if already exists
    if (this._chats.some(c => c.id === chat.id)) {
      this._chats = this._chats.map(c => c.id === chat.id ? chat : c);
    } else {
      this._chats = [...this._chats, chat];
    }
  }

  /**
   * Handle chat updated event.
   */
  #handleChatUpdated(chat: Chat): void {
    this._chats = this._chats.map(c => c.id === chat.id ? chat : c);
  }

  /**
   * Handle chat deleted event.
   */
  #handleChatDeleted(chatId: number): void {
    this._chats = this._chats.filter(c => c.id !== chatId);
  }
}