import type { ChatAction } from './types';
import {
  loadChats,
  loadAvailableModels,
  createChat,
  selectChat,
  deleteChat,
  deleteAllChats,
  sendMessage,
  editMessage,
  abortChat,
  pauseChat,
  resumeChat,
  handleChatEvent,
  loadAvailableRoles,
  setRole,
  clearActiveRole,
} from '../chatWs';
import { selectedModel, availableRoles, activeRole } from '../chatStores';

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
    case 'deleteAllChats':
      await deleteAllChats();
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
    case 'loadAvailableRoles':
      await loadAvailableRoles(action.payload.chatId);
      break;
    case 'setRole':
      await setRole(
        action.payload.chatId,
        action.payload.projectName,
        action.payload.roleName
      );
      break;
    case 'clearActiveRole':
      await clearActiveRole(action.payload.chatId);
      break;
    case 'roleChanged':
      activeRole.set({
        projectName: action.payload.projectName,
        roleName: action.payload.roleName,
      });
      break;
    case 'rolesUpdated':
      availableRoles.set(action.payload.roles);
      if (action.payload.activeRoleProject && action.payload.activeRoleName) {
        activeRole.set({
          projectName: action.payload.activeRoleProject,
          roleName: action.payload.activeRoleName,
        });
      } else {
        activeRole.set(null);
      }
      break;
    case 'activeRoleCleared':
      activeRole.set(null);
      break;
  }
}
