import { writable, derived } from 'svelte/store';

export const chats = writable([]);
export const currentChatId = writable(null);
export const messages = writable([]);
export const streamingContent = writable('');
export const isStreaming = writable(false);
export const streamError = writable(null);

export const currentChat = derived(
  [chats, currentChatId],
  ([$chats, $currentChatId]) => $chats.find(c => c.id === $currentChatId)
);
