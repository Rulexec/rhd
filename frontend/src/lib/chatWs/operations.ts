import { get } from 'svelte/store';
import { sendRequest, generateRequestId } from '../ws';
import type { WsResponse } from '../types/ws';
import type { QueuedMessage } from '../types/index';
import {
  chats,
  currentChatId,
  messages,
  streamingContent,
  streamingThinkingContent,
  isStreaming,
  streamError,
  availableModels,
  selectedModel,
  streamingMessageId,
  availableRoles,
  activeRole,
  todoList,
  queuedMessages,
} from '../chatStores';
import {
  chatProjects,
  mcpStatuses,
  loadChatProjects,
} from '../projectStores';
import { mergeToolResults } from './parsers';

export async function loadChats(): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'listChats', id });
  if (response.success) {
    chats.set(response.data || []);
  }
  return response;
}

export async function loadAvailableModels(): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getAvailableModels', id });
  if (response.success) {
    availableModels.set(response.data || []);
  }
  return response;
}

export async function createChat(title: string): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'createChat', id, title });
  if (response.success) {
    const chatId = response.data.chatId;
    const newChat = {
      id: chatId,
      title,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      activeModel: null,
    };
    chats.update((list) => [...list, newChat]);
    currentChatId.set(chatId);
    messages.set([]);
    selectedModel.set(null);
    chatProjects.set([]);
    mcpStatuses.set([]);
  }
  return response;
}

export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(mergeToolResults(response.data.messages || []));
    streamingContent.set('');
    streamingThinkingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    chatProjects.set([]);
    mcpStatuses.set([]);
    availableRoles.set([]);
    activeRole.set(null);
    todoList.set([]);
    loadChatProjects(chatId);
    loadAvailableRoles(chatId);
  }
  return response;
}

export async function deleteChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'deleteChat', id, chatId });
  if (response.success) {
    chats.update((list) => list.filter((c) => c.id !== chatId));
    const currentId = get(currentChatId);
    if (currentId === chatId) {
      currentChatId.set(null);
      messages.set([]);
    }
  }
  return response;
}

export async function deleteAllChats(): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'deleteAllChats', id });
  if (response.success) {
    chats.set([]);
    currentChatId.set(null);
    messages.set([]);
    streamingContent.set('');
    streamingThinkingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(null);
    chatProjects.set([]);
    mcpStatuses.set([]);
    todoList.set([]);
  }
  return response;
}

export async function sendMessage(content: string, model: string): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const tempId = Date.now();
  const userMessage = {
    id: tempId,
    chatId,
    role: 'user' as const,
    content,
    createdAt: new Date().toISOString(),
    model,
  };
  messages.update((list) => [...list, userMessage]);

  isStreaming.set(true);
  streamingContent.set('');
  streamingThinkingContent.set('');
  streamError.set(null);
  streamingMessageId.set(null);

  const id = generateRequestId();
  const response = await sendRequest({ type: 'sendMessage', id, chatId, content, model });
  if (!response.success) {
    streamError.set(response.error || 'Failed to send message');
    isStreaming.set(false);
  }
  return response;
}

export async function editMessage(messageId: number, newContent: string, model: string): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  messages.update((list) => {
    const idx = list.findIndex((m) => m.id === messageId);
    if (idx !== -1) {
      const updated = [...list];
      updated[idx] = { ...updated[idx], content: newContent };
      return updated.slice(0, idx + 1);
    }
    return list;
  });

  isStreaming.set(true);
  streamingContent.set('');
  streamingThinkingContent.set('');
  streamError.set(null);
  streamingMessageId.set(null);

  const id = generateRequestId();
  const response = await sendRequest({ type: 'editMessage', id, messageId, content: newContent, model });
  if (!response.success) {
    streamError.set(response.error || 'Failed to edit message');
    isStreaming.set(false);
  }
  return response;
}

export async function abortChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'abortChat', id, chatId });
  return response;
}

export async function loadAvailableRoles(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getAvailableRoles', id, chatId });
  if (response.success) {
    availableRoles.set(response.data.roles || []);
    if (response.data.activeRoleProject && response.data.activeRoleName) {
      activeRole.set({
        projectName: response.data.activeRoleProject,
        roleName: response.data.activeRoleName,
      });
    } else {
      activeRole.set(null);
    }
  }
  return response;
}

export async function setRole(
  chatId: number,
  projectName: string,
  roleName: string
): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({
    type: 'setRole',
    id,
    chatId,
    projectName,
    roleName,
  });
  if (response.success) {
    activeRole.set({ projectName, roleName });
  }
  return response;
}

export async function clearActiveRole(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'clearActiveRole', id, chatId });
  if (response.success) {
    activeRole.set(null);
  }
  return response;
}

export async function pauseChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'pauseChat', id, chatId });
  return response;
}

export async function resumeChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'resumeChat', id, chatId });
  return response;
}

export async function queueMessage(content: string, model: string): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const tempId = `queued-${Date.now()}`;
  const queuedMessage: QueuedMessage = {
    id: tempId,
    content,
    model,
    queuedAt: new Date().toISOString(),
    status: 'queued',
  };
  
  queuedMessages.update((list) => [...list, queuedMessage]);

  const id = generateRequestId();
  const response = await sendRequest({ type: 'queueMessage', id, chatId, content, model });
  if (!response.success) {
    queuedMessages.update((list) => list.filter((m) => m.id !== tempId));
  }
  return response;
}
