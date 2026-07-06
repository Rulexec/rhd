import { get } from 'svelte/store';
import { sendRequest, generateRequestId } from './ws';
import type { WsResponse } from './types/ws';
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
  isPaused,
  pendingToolCalls,
} from './chatStores';
import {
  handleMcpStatusEvent,
  handleProjectAttachedEvent,
  handleProjectDetachedEvent,
  loadChatProjects,
} from './projectStores';
import type { WsEvent } from './types/ws';

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
  }
  return response;
}

export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(response.data.messages || []);
    streamingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    loadChatProjects(chatId);
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

export function handleChatEvent(event: string, data: unknown): void {
  switch (event) {
    case 'chatStreamChunk': {
      const chunk = data as { content: string };
      const tempId = get(streamingMessageId);

      if (!tempId) {
        const chatId = get(currentChatId);
        if (!chatId) break;

        const newTempId = `temp-${Date.now()}`;
        const assistantMessage = {
          id: newTempId,
          chatId,
          role: 'assistant' as const,
          content: chunk.content,
          createdAt: new Date().toISOString(),
          model: get(selectedModel) || '',
          thinkingContent: get(streamingThinkingContent) || undefined,
        };
        messages.update((list) => [...list, assistantMessage as any]);
        streamingMessageId.set(newTempId);
      } else {
        messages.update((list) =>
          list.map((m) =>
            m.id === tempId ? { ...m, content: m.content + chunk.content } : m
          )
        );
      }
      break;
    }
    case 'chatThinkingChunk': {
      const chunk = data as { content: string };
      streamingThinkingContent.update((current) => current + chunk.content);
      
      const tempId = get(streamingMessageId);
      if (tempId) {
        messages.update((list) =>
          list.map((m) =>
            m.id === tempId ? { ...m, thinkingContent: (m.thinkingContent || '') + chunk.content } : m
          )
        );
      }
      break;
    }
    case 'chatStreamFinished': {
      isStreaming.set(false);
      streamError.set(null);
      break;
    }
    case 'chatStreamError': {
      const error = data as { error: string };
      streamError.set(error.error);
      isStreaming.set(false);
      streamingMessageId.set(null);
      break;
    }
    case 'chatMessageAdded': {
      const added = data as { message: { id: number; role: string; content: string } };
      const tempId = get(streamingMessageId);

      messages.update((list) => {
        if (added.message.role === 'assistant' && tempId) {
          const tempIdx = list.findIndex((m) => m.id === tempId);
          if (tempIdx !== -1) {
            const updated = [...list];
            updated[tempIdx] = added.message as any;
            streamingMessageId.set(null);
            return updated;
          } else {
            // Temp message not found, clear streamingMessageId anyway
            streamingMessageId.set(null);
          }
        }

        if (list.some((m) => m.id === added.message.id)) {
          return list;
        }
        
        // System message: insert before temp user message
        if (added.message.role === 'system') {
          const tempUserIdx = list.findIndex(
            (m) => m.role === 'user' && ((m.id as number) < 0 || (m.id as number) > 1000000000000)
          );
          if (tempUserIdx !== -1) {
            const updated = [...list];
            updated.splice(tempUserIdx, 0, added.message as any);
            return updated;
          }
        }
        
        if (added.message.role === 'user') {
          const tempIdx = list.findIndex(
            (m) => m.role === 'user' && m.content === added.message.content && ((m.id as number) < 0 || (m.id as number) > 1000000000000)
          );
          if (tempIdx !== -1) {
            const updated = [...list];
            updated[tempIdx] = added.message as any;
            return updated;
          }
        }
        return [...list, added.message as any];
      });
      break;
    }
    case 'chatUpdated': {
      const updated = data as { chatId: number; title: string };
      chats.update((list) =>
        list.map((c) => (c.id === updated.chatId ? { ...c, title: updated.title } : c))
      );
      break;
    }
    case 'projectMcpStatusChanged': {
      handleMcpStatusEvent(data);
      break;
    }
    case 'projectAttached': {
      handleProjectAttachedEvent(data);
      break;
    }
    case 'projectDetached': {
      handleProjectDetachedEvent(data);
      break;
    }
    case 'toolCallStarted': {
      const { toolCallId, toolName, arguments: args, mcpName } = data as {
        chatId: number;
        toolCallId: string;
        toolName: string;
        arguments: string;
        mcpName: string;
      };
      const newToolCall = {
        id: toolCallId,
        name: toolName,
        arguments: args,
        status: 'running' as const,
        mcpName,
      };
      pendingToolCalls.update((list) => [...list, newToolCall]);
      
      const tempId = get(streamingMessageId);
      if (tempId) {
        messages.update((list) =>
          list.map((m) => {
            if (m.id === tempId) {
              const existingToolCalls = m.toolCalls || [];
              return { ...m, toolCalls: [...existingToolCalls, newToolCall] };
            }
            return m;
          })
        );
      }
      break;
    }
    case 'toolCallCompleted': {
      const { toolCallId, result } = data as {
        chatId: number;
        toolCallId: string;
        result: string;
      };
      pendingToolCalls.update((list) =>
        list.map((tc) =>
          tc.id === toolCallId ? { ...tc, result, status: 'completed' as const } : tc
        )
      );
      
      const tempId = get(streamingMessageId);
      if (tempId) {
        messages.update((list) =>
          list.map((m) => {
            if (m.id === tempId && m.toolCalls) {
              return {
                ...m,
                toolCalls: m.toolCalls.map((tc) =>
                  tc.id === toolCallId ? { ...tc, result, status: 'completed' as const } : tc
                ),
              };
            }
            return m;
          })
        );
      }
      break;
    }
    case 'chatPaused': {
      isPaused.set(true);
      isStreaming.set(false);
      break;
    }
    case 'chatResumed': {
      isPaused.set(false);
      isStreaming.set(true);
      break;
    }
  }
}
