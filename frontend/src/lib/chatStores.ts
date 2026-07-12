import { writable, derived } from 'svelte/store';
import type { Writable, Readable } from 'svelte/store';
import type { Chat, ChatMessage, ToolCall, RoleInfo, ActiveRole } from './types/index';

export const chats: Writable<Chat[]> = writable([]);
export const currentChatId: Writable<number | null> = writable(null);
export const messages: Writable<ChatMessage[]> = writable([]);
export const streamingContent: Writable<string> = writable('');
export const streamingThinkingContent: Writable<string> = writable('');
export const isStreaming: Writable<boolean> = writable(false);
export const streamError: Writable<string | null> = writable(null);
export const availableModels: Writable<string[]> = writable([]);
export const selectedModel: Writable<string | null> = writable(null);
export const streamingMessageId: Writable<string | null> = writable(null);
export const isPaused: Writable<boolean> = writable(false);
export const pendingToolCalls: Writable<ToolCall[]> = writable([]);
export const availableRoles: Writable<RoleInfo[]> = writable([]);
export const activeRole: Writable<ActiveRole | null> = writable(null);

export const currentChat: Readable<Chat | undefined> = derived(
  [chats, currentChatId],
  ([$chats, $currentChatId]) => $chats.find(c => c.id === $currentChatId)
);

export const isToolLoopRunning: Readable<boolean> = derived(
  [isStreaming, isPaused],
  ([$isStreaming, $isPaused]) => $isStreaming && !$isPaused
);

export const hasRoles: Readable<boolean> = derived(
  availableRoles,
  ($availableRoles) => $availableRoles.length > 0
);

export function resetAllStores(): void {
  chats.set([]);
  currentChatId.set(null);
  messages.set([]);
  streamingContent.set('');
  streamingThinkingContent.set('');
  isStreaming.set(false);
  streamError.set(null);
  availableModels.set([]);
  selectedModel.set(null);
  streamingMessageId.set(null);
  isPaused.set(false);
  pendingToolCalls.set([]);
  availableRoles.set([]);
  activeRole.set(null);
}
