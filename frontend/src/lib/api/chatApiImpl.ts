import { z } from 'zod';
import { websocket } from './websocket.js';
import {
  ListChatsResultSchema,
  CreateChatResultSchema,
  GetChatResultSchema,
  GetQueueMessagesResultSchema,
  GetPluginsResultSchema,
  GetToolsResultSchema,
  UpdateChatResultSchema,
  ChatCreatedDataSchema,
  ChatUpdatedDataSchema,
  ChatDeletedDataSchema,
  MessageAddedDataSchema,
  MessageUpdatedDataSchema,
  MessageDeletedDataSchema,
  QueueMessageAddedDataSchema,
  QueueMessageUpdatedDataSchema,
  QueueMessageDeletedDataSchema,
  PluginRegisteredDataSchema,
  PluginUpdatedDataSchema,
  PluginRemovedDataSchema,
  GetPluginStatesResultSchema,
  SubscribePluginStatesResultSchema,
  PluginStateChangedDataSchema,
  PluginStateRemovedDataSchema,
  StreamChunkDataSchema,
  StreamFinishedDataSchema,
  StreamSubscribeResultSchema,
  ToolsUpdatedDataSchema,
  type ListChatsResult,
  type CreateChatResult,
  type GetChatResult,
  type GetQueueMessagesResult,
  type GetPluginsResult,
  type GetToolsResult,
  type UpdateChatParams,
  type UpdateChatResult,
  type ChatCreatedData,
  type ChatUpdatedData,
  type ChatDeletedData,
  type MessageAddedData,
  type MessageUpdatedData,
  type MessageDeletedData,
  type QueueMessageAddedData,
  type QueueMessageUpdatedData,
  type QueueMessageDeletedData,
  type PluginRegisteredData,
  type PluginUpdatedData,
  type PluginRemovedData,
  type GetPluginStatesResult,
  type SubscribePluginStatesResult,
  type PluginStateChangedData,
  type PluginStateRemovedData,
  type StateVersionRef,
  type StreamChunkData,
  type StreamFinishedData,
  type StreamSubscribeResult,
  type ToolsUpdatedData
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
  try {
    return CreateChatResultSchema.parse(data);
  } catch (error) {
    if (error instanceof z.ZodError) {
      console.error('CreateChat validation errors:', error.errors);
      console.error('Raw data:', data);
    }
    throw error;
  }
}

/**
 * Delete a chat and all its messages.
 * @param chatId - Chat ID to delete
 */
export async function deleteChat(chatId: number): Promise<void> {
  await websocket.request('deleteChat', { chatId });
}

/**
 * Update a chat (title, tags).
 * @param params - Update parameters
 */
export async function updateChat(params: UpdateChatParams): Promise<UpdateChatResult> {
  const data = await websocket.request('updateChat', params);
  return UpdateChatResultSchema.parse(data);
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
  onChatUpdated?: (data: ChatUpdatedData) => void;
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

  if (handlers.onChatUpdated) {
    unsubs.push(websocket.on('chatUpdated', (data) => {
      const parsed = ChatUpdatedDataSchema.parse(data);
      if (parsed.chat.id === chatId) {
        handlers.onChatUpdated!(parsed);
      }
    }));
  }

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

/**
 * Get queue messages for a chat.
 * @param chatId - Chat ID
 */
export async function getQueueMessages(chatId: number): Promise<GetQueueMessagesResult> {
  const data = await websocket.request('getQueueMessages', { chatId });
  return GetQueueMessagesResultSchema.parse(data);
}

/**
 * Add a message to the queue.
 * @param chatId - Chat ID
 * @param role - Message role (e.g., "user")
 * @param content - Message content
 * @param tags - Optional tags
 */
export async function addQueueMessage(
  chatId: number,
  role: string,
  content: string,
  tags: string[] = []
): Promise<void> {
  await websocket.request('addQueueMessage', {
    chatId,
    role,
    content,
    tags
  });
}

export interface QueueMessageEventHandlers {
  onQueueMessageAdded?: (data: QueueMessageAddedData) => void;
  onQueueMessageUpdated?: (data: QueueMessageUpdatedData) => void;
  onQueueMessageDeleted?: (data: QueueMessageDeletedData) => void;
}

/**
 * Register event listeners for queue message events.
 * @param chatId - Chat ID to listen for
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onQueueMessageEvents(
  chatId: number,
  handlers: QueueMessageEventHandlers
): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onQueueMessageAdded) {
    unsubs.push(websocket.on('queueMessageAdded', (data) => {
      const parsed = QueueMessageAddedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onQueueMessageAdded!(parsed);
      }
    }));
  }

  if (handlers.onQueueMessageUpdated) {
    unsubs.push(websocket.on('queueMessageUpdated', (data) => {
      const parsed = QueueMessageUpdatedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onQueueMessageUpdated!(parsed);
      }
    }));
  }

  if (handlers.onQueueMessageDeleted) {
    unsubs.push(websocket.on('queueMessageDeleted', (data) => {
      const parsed = QueueMessageDeletedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onQueueMessageDeleted!(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}

/**
 * Subscribe to plugin list events (pluginRegistered, pluginUpdated, pluginRemoved).
 * Must be called before getPlugins to receive real-time updates.
 */
export async function subscribePluginsList(): Promise<void> {
  await websocket.request('subscribePluginsList', {});
}

/**
 * Unsubscribe from plugin list events.
 */
export async function unsubscribePluginsList(): Promise<void> {
  await websocket.request('unsubscribePluginsList', {});
}

/**
 * Get list of all registered plugins.
 */
export async function getPlugins(): Promise<GetPluginsResult> {
  const data = await websocket.request('getPlugins', {});
  return GetPluginsResultSchema.parse(data);
}

export interface PluginListEventHandlers {
  onPluginRegistered?: (data: PluginRegisteredData) => void;
  onPluginUpdated?: (data: PluginUpdatedData) => void;
  onPluginRemoved?: (data: PluginRemovedData) => void;
}

/**
 * Register event listeners for plugin list events.
 * Returns cleanup function.
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onPluginListEvents(handlers: PluginListEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onPluginRegistered) {
    unsubs.push(websocket.on('pluginRegistered', (data) => {
      const parsed = PluginRegisteredDataSchema.parse(data);
      handlers.onPluginRegistered!(parsed);
    }));
  }

  if (handlers.onPluginUpdated) {
    unsubs.push(websocket.on('pluginUpdated', (data) => {
      const parsed = PluginUpdatedDataSchema.parse(data);
      handlers.onPluginUpdated!(parsed);
    }));
  }

  if (handlers.onPluginRemoved) {
    unsubs.push(websocket.on('pluginRemoved', (data) => {
      const parsed = PluginRemovedDataSchema.parse(data);
      handlers.onPluginRemoved!(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}

// ============================================================================
// Plugin State API Methods
// ============================================================================

/**
 * Get plugin states, optionally filtered by pluginId and/or schema.
 * @param filters - Optional pluginId and schema filters
 */
export async function getPluginStates(filters?: {
  pluginId?: string;
  schema?: string;
}): Promise<GetPluginStatesResult> {
  const data = await websocket.request('getPluginStates', filters ?? {});
  return GetPluginStatesResultSchema.parse(data);
}

/**
 * Subscribe to plugin state events and atomically catch up on states newer
 * than the passed versions. Pass `[]` to subscribe without catch-up.
 * The server registers the subscription before snapshotting, so no update
 * can be lost between getPluginStates and this call (duplicates are handled
 * by version-gating in the store).
 * @param refs - Versions the client currently holds per (pluginId, key)
 */
export async function subscribePluginStates(
  refs: StateVersionRef[]
): Promise<SubscribePluginStatesResult> {
  const data = await websocket.request('subscribePluginStates', { states: refs });
  return SubscribePluginStatesResultSchema.parse(data);
}

/**
 * Unsubscribe from plugin state events.
 */
export async function unsubscribePluginStates(): Promise<void> {
  await websocket.request('unsubscribePluginStates', {});
}

export interface PluginStateEventHandlers {
  onPluginStateChanged?: (data: PluginStateChangedData) => void;
  onPluginStateRemoved?: (data: PluginStateRemovedData) => void;
}

/**
 * Register event listeners for plugin state events.
 * Returns cleanup function.
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onPluginStateEvents(handlers: PluginStateEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onPluginStateChanged) {
    unsubs.push(websocket.on('pluginStateChanged', (data) => {
      const parsed = PluginStateChangedDataSchema.parse(data);
      handlers.onPluginStateChanged!(parsed);
    }));
  }

  if (handlers.onPluginStateRemoved) {
    unsubs.push(websocket.on('pluginStateRemoved', (data) => {
      const parsed = PluginStateRemovedDataSchema.parse(data);
      handlers.onPluginStateRemoved!(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}

// ============================================================================
// Tools API Methods
// ============================================================================

/**
 * Get all tools registered for a chat.
 * @param chatId - Chat ID to get tools for
 */
export async function getTools(chatId: number): Promise<GetToolsResult> {
  const data = await websocket.request('getTools', { chatId });
  return GetToolsResultSchema.parse(data);
}

export interface ToolsEventHandlers {
  onToolsUpdated?: (data: ToolsUpdatedData) => void;
}

/**
 * Register event listeners for tools events.
 * @param chatId - Chat ID to listen for
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onToolsEvents(chatId: number, handlers: ToolsEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onToolsUpdated) {
    unsubs.push(websocket.on('toolsUpdated', (data) => {
      const parsed = ToolsUpdatedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onToolsUpdated!(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}

// ============================================================================
// Stream API Methods
// ============================================================================

/**
 * Subscribe to a stream and get current accumulated content.
 * @param chatId - Chat ID to subscribe to
 */
export async function streamSubscribe(chatId: number): Promise<StreamSubscribeResult> {
  const data = await websocket.request('streamSubscribe', { chatId });
  return StreamSubscribeResultSchema.parse(data);
}

export interface StreamEventHandlers {
  onStreamChunk?: (data: StreamChunkData) => void;
  onStreamFinished?: (data: StreamFinishedData) => void;
}

/**
 * Register event listeners for stream events.
 * @param chatId - Chat ID to listen for
 * @param handlers - Event handlers
 * @returns Cleanup function that removes all listeners
 */
export function onStreamEvents(chatId: number, handlers: StreamEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onStreamChunk) {
    unsubs.push(websocket.on('streamChunk', (data) => {
      const parsed = StreamChunkDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onStreamChunk!(parsed);
      }
    }));
  }

  if (handlers.onStreamFinished) {
    unsubs.push(websocket.on('streamFinished', (data) => {
      const parsed = StreamFinishedDataSchema.parse(data);
      if (parsed.chatId === chatId) {
        handlers.onStreamFinished!(parsed);
      }
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
