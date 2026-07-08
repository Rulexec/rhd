import type { ChatAction } from './types';
import {
  loadChats,
  loadAvailableModels,
  createChat,
  selectChat,
  deleteChat,
  sendMessage,
  editMessage,
  abortChat,
  pauseChat,
  resumeChat,
  handleChatEvent,
} from '../chatWs';
import { selectedModel } from '../chatStores';

export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    case 'loadChats':
      await loadChats();
      break;
    case 'loadAvailableModels':
      await loadAvailableModels();
      break;
    case 'createChat':
      await createChat(action.payload.title);
      break;
    case 'selectChat':
      await selectChat(action.payload.chatId);
      break;
    case 'deleteChat':
      await deleteChat(action.payload.chatId);
      break;
    case 'sendMessage':
      await sendMessage(action.payload.content, action.payload.model);
      break;
    case 'editMessage':
      await editMessage(action.payload.messageId, action.payload.content, action.payload.model);
      break;
    case 'abortChat':
      await abortChat();
      break;
    case 'pauseChat':
      await pauseChat();
      break;
    case 'resumeChat':
      await resumeChat();
      break;
    case 'selectModel':
      selectedModel.set(action.payload.model);
      break;
    case 'chatStreamChunk':
      handleChatEvent('chatStreamChunk', action.payload);
      break;
    case 'chatThinkingChunk':
      handleChatEvent('chatThinkingChunk', action.payload);
      break;
    case 'chatStreamFinished':
      handleChatEvent('chatStreamFinished', undefined);
      break;
    case 'chatStreamError':
      handleChatEvent('chatStreamError', action.payload);
      break;
    case 'chatMessageAdded':
      handleChatEvent('chatMessageAdded', action.payload);
      break;
    case 'chatUpdated':
      handleChatEvent('chatUpdated', action.payload);
      break;
    case 'chatToolCallStarted':
      handleChatEvent('chatToolCallStarted', action.payload);
      break;
    case 'chatToolCallCompleted':
      handleChatEvent('chatToolCallCompleted', action.payload);
      break;
    case 'chatPaused':
      handleChatEvent('chatPaused', undefined);
      break;
    case 'chatResumed':
      handleChatEvent('chatResumed', undefined);
      break;
    case 'projectMcpStatusChanged':
      handleChatEvent('projectMcpStatusChanged', action.payload);
      break;
    case 'projectAttached':
      handleChatEvent('projectAttached', action.payload);
      break;
    case 'projectDetached':
      handleChatEvent('projectDetached', action.payload);
      break;
  }
}
