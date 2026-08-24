import { writable, derived, type Writable, type Readable } from 'svelte/store';
import { subscribeChatsList, listChats, createChat, deleteChat, generateChatTitle, onChatListEvents } from '../api/chatApi.js';
import type { Chat } from '../api/schemas.js';
import { selectChat } from './chat.js';

/**
 * Internal store for raw chat list.
 */
const _chats: Writable<Chat[]> = writable([]);

/**
 * Loading state.
 */
export const chatsLoading: Writable<boolean> = writable(false);

/**
 * Error state.
 */
export const chatsError: Writable<string | null> = writable(null);

/**
 * Derived store: chats sorted by updatedAt DESC.
 */
export const chats: Readable<Chat[]> = derived(_chats, ($chats) => {
  return [...$chats].sort((a, b) => {
    return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
  });
});

/**
 * Whether there are any chats.
 */
export const hasChats: Readable<boolean> = derived(chats, ($chats) => $chats.length > 0);

/**
 * Initialize the chats store: subscribe to events and load initial data.
 * Should be called once on app startup.
 */
export async function initChats(): Promise<void> {
  chatsError.set(null);

  try {
    // Subscribe to chat list events
    await subscribeChatsList();

    // Register event listeners
    onChatListEvents({
      onChatCreated: ({ chat }) => {
        _chats.update(current => {
          // Don't add if already exists
          if (current.some(c => c.id === chat.id)) {
            return current.map(c => c.id === chat.id ? chat : c);
          }
          return [...current, chat];
        });
      },
      onChatUpdated: ({ chat }) => {
        _chats.update(current =>
          current.map(c => c.id === chat.id ? chat : c)
        );
      },
      onChatDeleted: ({ chatId }) => {
        _chats.update(current =>
          current.filter(c => c.id !== chatId)
        );
      }
    });

    // Load initial chat list
    await loadChats();
  } catch (error) {
    chatsError.set(error instanceof Error ? error.message : String(error));
  }
}

/**
 * Load chats from server.
 */
export async function loadChats(): Promise<void> {
  chatsLoading.set(true);
  chatsError.set(null);

  try {
    const result = await listChats();
    _chats.set(result.chats);
  } catch (error) {
    chatsError.set(error instanceof Error ? error.message : String(error));
  } finally {
    chatsLoading.set(false);
  }
}

/**
 * Create a new chat with auto-generated title.
 * @returns The created chat, or null on error
 */
export async function createNewChat(): Promise<number | null> {
  chatsError.set(null);

  try {
    const title = generateChatTitle();
    const result = await createChat(title);
    // Chat will be added via chatCreated event
    // Automatically open the new chat
    await selectChat(result.chatId);
    return result.chatId;
  } catch (error) {
    chatsError.set(error instanceof Error ? error.message : String(error));
    return null;
  }
}

/**
 * Delete all chats.
 * @returns True if successful
 */
export async function deleteAllChats(): Promise<boolean> {
  chatsError.set(null);

  try {
    // Get current chat IDs
    let currentChats: Chat[] = [];
    chats.subscribe(value => { currentChats = value; })();

    if (!currentChats || currentChats.length === 0) {
      return true;
    }

    // Delete each chat
    for (const chat of currentChats) {
      await deleteChat(chat.id);
    }

    return true;
  } catch (error) {
    chatsError.set(error instanceof Error ? error.message : String(error));
    return false;
  }
}

/**
 * Clear the chats store.
 */
export function clearChats(): void {
  _chats.set([]);
  chatsLoading.set(false);
  chatsError.set(null);
}
