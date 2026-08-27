import * as chatApi from './chatApiImpl.js';

/**
 * Chat API interface for dependency injection.
 * Allows mocking in tests.
 *
 * Streaming functions (streamSubscribe / onStreamEvents) are intentionally
 * NOT part of this interface: components import them directly from chatApiImpl.
 */
export interface ChatApi {
  subscribeChatsList: typeof chatApi.subscribeChatsList;
  unsubscribeChatsList: typeof chatApi.unsubscribeChatsList;
  listChats: typeof chatApi.listChats;
  createChat: typeof chatApi.createChat;
  deleteChat: typeof chatApi.deleteChat;
  generateChatTitle: typeof chatApi.generateChatTitle;
  onChatListEvents: typeof chatApi.onChatListEvents;
  subscribeChat: typeof chatApi.subscribeChat;
  unsubscribeChat: typeof chatApi.unsubscribeChat;
  getChat: typeof chatApi.getChat;
  onChatEvents: typeof chatApi.onChatEvents;
  getQueueMessages: typeof chatApi.getQueueMessages;
  addQueueMessage: typeof chatApi.addQueueMessage;
  onQueueMessageEvents: typeof chatApi.onQueueMessageEvents;
  subscribePluginsList: typeof chatApi.subscribePluginsList;
  getPlugins: typeof chatApi.getPlugins;
  onPluginListEvents: typeof chatApi.onPluginListEvents;
}

/**
 * Default ChatApi implementation using actual API functions.
 */
export const defaultChatApi: ChatApi = {
  subscribeChatsList: chatApi.subscribeChatsList,
  unsubscribeChatsList: chatApi.unsubscribeChatsList,
  listChats: chatApi.listChats,
  createChat: chatApi.createChat,
  deleteChat: chatApi.deleteChat,
  generateChatTitle: chatApi.generateChatTitle,
  onChatListEvents: chatApi.onChatListEvents,
  subscribeChat: chatApi.subscribeChat,
  unsubscribeChat: chatApi.unsubscribeChat,
  getChat: chatApi.getChat,
  onChatEvents: chatApi.onChatEvents,
  getQueueMessages: chatApi.getQueueMessages,
  addQueueMessage: chatApi.addQueueMessage,
  onQueueMessageEvents: chatApi.onQueueMessageEvents,
  subscribePluginsList: chatApi.subscribePluginsList,
  getPlugins: chatApi.getPlugins,
  onPluginListEvents: chatApi.onPluginListEvents,
};