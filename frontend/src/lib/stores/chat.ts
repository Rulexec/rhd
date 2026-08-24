import { writable, derived, type Writable, type Readable } from 'svelte/store';
import { getChat, subscribeChat, unsubscribeChat, onChatEvents, getQueueMessages, onQueueMessageEvents } from '../api/chatApi.js';
import type { Chat, Message } from '../api/schemas.js';

/**
 * Current chat ID.
 */
export const currentChatId: Writable<number | null> = writable(null);

/**
 * Current chat metadata.
 */
export const currentChat: Writable<Chat | null> = writable(null);

/**
 * Messages for the current chat.
 */
export const messages: Writable<Message[]> = writable([]);

/**
 * Queue messages for the current chat.
 */
export const queueMessages: Writable<Message[]> = writable([]);

/**
 * Loading state.
 */
export const chatLoading: Writable<boolean> = writable(false);

/**
 * Error state.
 */
export const chatError: Writable<string | null> = writable(null);

/**
 * Cleanup function for event listeners.
 */
let cleanupEvents: (() => void) | null = null;

/**
 * Cleanup function for queue event listeners.
 */
let cleanupQueueEvents: (() => void) | null = null;

/**
 * Select a chat and load its messages.
 * @param chatId - Chat ID to select
 */
export async function selectChat(chatId: number): Promise<void> {
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
    chatError.set(error instanceof Error ? error.message : String(error));
  } finally {
    chatLoading.set(false);
  }
}

/**
 * Clear the current chat and unsubscribe.
 */
export async function clearChat(): Promise<void> {
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
  const prevChatId = await new Promise<number | null>(resolve => {
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
 * Message with queue flag for display.
 */
export interface DisplayMessage extends Message {
  isQueue: boolean;
}

/**
 * Derived store: all messages (regular + queue) sorted by createdAt ASC.
 * Queue messages are marked with isQueue flag.
 */
export const allMessages: Readable<DisplayMessage[]> = derived(
  [messages, queueMessages],
  ([$messages, $queueMessages]) => {
    const regular: DisplayMessage[] = $messages.map(m => ({ ...m, isQueue: false }));
    const queue: DisplayMessage[] = $queueMessages.map(m => ({ ...m, isQueue: true }));

    return [...regular, ...queue].sort((a, b) => {
      return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
    });
  }
);
