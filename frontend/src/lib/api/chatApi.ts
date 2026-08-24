import { websocket } from './websocket.js';
import {
  ListChatsResultSchema,
  CreateChatResultSchema,
  GetChatResultSchema,
  ChatCreatedDataSchema,
  ChatUpdatedDataSchema,
  ChatDeletedDataSchema,
  MessageAddedDataSchema,
  MessageUpdatedDataSchema,
  MessageDeletedDataSchema,
  type ListChatsResult,
  type CreateChatResult,
  type GetChatResult,
  type ChatCreatedData,
  type ChatUpdatedData,
  type ChatDeletedData,
  type MessageAddedData,
  type MessageUpdatedData,
  type MessageDeletedData
} from './schemas.js';

/**
 * Subscribe to chat list events (chatCreated, chatUpdated, chatDeleted).
 * Must be called before listChats to receive real-time updates.
 */
export async function subscribeChatsList(): Promise<void> {
  await websocket.request('subscribeChatsList', {});
}

/**
 * Unsubscribe from chat list events.
 */
export async function unsubscribeChatsList(): Promise<void> {
  await websocket.request('unsubscribeChatsList', {});
}

/**
 * Get list of all chats sorted by updatedAt DESC.
 */
export async function listChats(): Promise<ListChatsResult> {
  const data = await websocket.request('listChats', {});
  return ListChatsResultSchema.parse(data);
}

/**
 * Create a new chat with the given title.
 * @param title - Chat title
 */
export async function createChat(title: string): Promise<CreateChatResult> {
  const data = await websocket.request('createChat', { title });
  return CreateChatResultSchema.parse(data);
}

/**
 * Delete a chat and all its messages.
 * @param chatId - Chat ID to delete
 */
export async function deleteChat(chatId: number): Promise<void> {
  await websocket.request('deleteChat', { chatId });
}

/**
 * Generate chat title from current date/time.
 * Format: YYYY-MM-DD HH:mm
 */
export function generateChatTitle(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const hours = String(now.getHours()).padStart(2, '0');
  const minutes = String(now.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

export interface ChatListEventHandlers {
  onChatCreated?: (data: ChatCreatedData) => void;
  onChatUpdated?: (data: ChatUpdatedData) => void;
  onChatDeleted?: (data: ChatDeletedData) => void;
}

/**
 * Register event listeners for chat list events.
 * Returns cleanup functions.
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onChatListEvents(handlers: ChatListEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onChatCreated) {
    unsubs.push(websocket.on('chatCreated', (data) => {
      const parsed = ChatCreatedDataSchema.parse(data);
      handlers.onChatCreated!(parsed);
    }));
  }

  if (handlers.onChatUpdated) {
    unsubs.push(websocket.on('chatUpdated', (data) => {
      const parsed = ChatUpdatedDataSchema.parse(data);
      handlers.onChatUpdated!(parsed);
    }));
  }

  if (handlers.onChatDeleted) {
    unsubs.push(websocket.on('chatDeleted', (data) => {
      const parsed = ChatDeletedDataSchema.parse(data);
      handlers.onChatDeleted!(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}

/**
 * Subscribe to chat events for a specific chat.
 * @param chatId - Chat ID to subscribe to
 */
export async function subscribeChat(chatId: number): Promise<void> {
  await websocket.request('subscribeChat', { chatId });
}

/**
 * Unsubscribe from chat events.
 * @param chatId - Chat ID to unsubscribe from
 */
export async function unsubscribeChat(chatId: number): Promise<void> {
  await websocket.request('unsubscribeChat', { chatId });
}

/**
 * Get chat details with messages.
 * @param chatId - Chat ID
 */
export async function getChat(chatId: number): Promise<GetChatResult> {
  const data = await websocket.request('getChat', { chatId });
  return GetChatResultSchema.parse(data);
}

export interface ChatEventHandlers {
  onMessageAdded?: (data: MessageAddedData) => void;
  onMessageUpdated?: (data: MessageUpdatedData) => void;
  onMessageDeleted?: (data: MessageDeletedData) => void;
}

/**
 * Register event listeners for chat-specific events.
 * @param chatId - Chat ID to listen for
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onChatEvents(chatId: number, handlers: ChatEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onMessageAdded) {
    unsubs.push(websocket.on('messageAdded', (data) => {
      const parsed = MessageAddedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onMessageAdded!(parsed);
      }
    }));
  }

  if (handlers.onMessageUpdated) {
    unsubs.push(websocket.on('messageUpdated', (data) => {
      const parsed = MessageUpdatedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onMessageUpdated!(parsed);
      }
    }));
  }

  if (handlers.onMessageDeleted) {
    unsubs.push(websocket.on('messageDeleted', (data) => {
      const parsed = MessageDeletedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onMessageDeleted!(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
