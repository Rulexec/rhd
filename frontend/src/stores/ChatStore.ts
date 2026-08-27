import { makeAutoObservable, flowResult } from 'mobx';
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
 * MobX store for the current chat state and message management.
 *
 * Async operations are written as generator PROTOTYPE methods. `makeAutoObservable`
 * auto-wraps generator methods into flows (actions wrapping a promise chain), so
 * calling e.g. `store.selectChat(1)` returns a CancellablePromise, not a suspended
 * generator. Where a flow needs an explicit promise from another auto-wrapped call
 * (e.g. `selectChat` internally calling `clearChat`), wrap the call with
 * `flowResult(...)` and `yield* yieldPromise(flowResult(...))`.
 *
 * State fields that drive reactive getters (`currentChatId`, `currentChat`,
 * `messages`, `queueMessages`, `loading`, `error`) are PUBLIC observable fields:
 * `#`-private class fields are NOT observable by MobX (see mobx-probe.test.ts).
 * Non-reactive dependencies (`#chatApi`, `#cleanupEvents`, `#cleanupQueueEvents`)
 * may stay private.
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
  *selectChat(chatId: number): Generator<unknown, void, unknown> {
    // Cleanup previous subscription
    yield* yieldPromise(flowResult(this.clearChat()));

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
   * Add a message to the queue.
   * @param chatId - Chat ID
   * @param role - Message role (e.g., "user")
   * @param content - Message content
   * @param tags - Optional tags
   */
  *addQueueMessage(chatId: number, role: string, content: string, tags: string[] = []): Generator<unknown, void, unknown> {
    try {
      yield* yieldPromise(this.#chatApi.addQueueMessage(chatId, role, content, tags));
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
      throw error;
    }
  }

  /**
   * Clear the current chat and unsubscribe.
   */
  *clearChat(): Generator<unknown, void, unknown> {
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