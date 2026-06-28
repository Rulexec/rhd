import { writable, derived } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';
import type { Chat, ChatMessage } from './types/index';

export const chats: Writable<Chat[]> = writable([]);
export const currentChatId: Writable<number | null> = writable(null);
export const messages: Writable<ChatMessage[]> = writable([]);
export const streamingContent: Writable<string> = writable('');
export const isStreaming: Writable<boolean> = writable(false);
export const streamError: Writable<string | null> = writable(null);
export const availableModels: Writable<string[]> = writable([]);
export const selectedModel: Writable<string | null> = writable(null);

export const currentChat: Readable<Chat | undefined> = derived(
  [chats, currentChatId],
  ([$chats, $currentChatId]) => $chats.find(c => c.id === $currentChatId)
);
