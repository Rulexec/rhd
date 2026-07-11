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
  chatProjects,
  mcpStatuses,
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
    messages.set(response.data.messages || []);
    streamingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    chatProjects.set([]);
    mcpStatuses.set([]);
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
      const added = data as { message: { id: number; role: string; content: string; thinkingContent?: string } };
      const tempId = get(streamingMessageId);
      messages.update((list) => {
        // Helper function to get numeric ID for ordering
        const getNumericId = (id: number | string): number => {
          if (typeof id === 'number') return id;
          if (typeof id === 'string' && id.startsWith('temp-')) {
            return Number.MAX_SAFE_INTEGER;
          }
          return parseInt(id as string, 10) || Number.MAX_SAFE_INTEGER;
        };

        // Check if message with same ID already exists
        if (list.some((m) => m.id === added.message.id)) {
          return list;
        }

        // Handle user message: replace temp user message with real one from backend
        if (added.message.role === 'user') {
          const tempUserIdx = list.findIndex(
            (m) => m.role === 'user' && m.content === added.message.content && typeof m.id === 'number' && m.id > 1000000000000
          );
          if (tempUserIdx !== -1) {
            const updated = [...list];
            updated[tempUserIdx] = added.message as any;
            return updated;
          }
          // No temp message found, add real message
          return [...list, added.message as any];
        }

        // Handle tool result messages: merge result into assistant message, don't add separate tool message
        if (added.message.role === 'tool') {
          try {
            const toolResult = JSON.parse(added.message.content);
            const toolCallId = toolResult.toolCallId;
            const result = toolResult.result;
            
            // Find the assistant message with this toolCallId and update it
            let updatedMessageId: number | string | null = null;
            const updated = list.map((m) => {
              if (m.role === 'assistant' && m.toolCalls) {
                const toolCallIndex = m.toolCalls.findIndex((tc) => tc.id === toolCallId);
                if (toolCallIndex !== -1) {
                  updatedMessageId = m.id;
                  const updatedToolCalls = [...m.toolCalls];
                  updatedToolCalls[toolCallIndex] = {
                    ...updatedToolCalls[toolCallIndex],
                    result,
                    status: 'completed' as const,
                  };
                  return { ...m, toolCalls: updatedToolCalls };
                }
              }
              return m;
            });
            return updated;
          } catch {
            // Not JSON, add as regular message
            return [...list, added.message as any];
          }
        }

        // Handle assistant message: replace temp message if exists
        if (added.message.role === 'assistant' && tempId) {
          const tempIdx = list.findIndex((m) => m.id === tempId);
          if (tempIdx !== -1) {
            const tempMsg = list[tempIdx];
            
            // Parse JSON content to extract toolCalls if present
            let parsedContent = added.message.content;
            let toolCalls: any[] | undefined;
            
            try {
              const json = JSON.parse(added.message.content);
              if (json.content !== undefined && json.toolCalls) {
                // Intermediate message with toolCalls - map to frontend format
                parsedContent = json.content;
                toolCalls = json.toolCalls.length > 0 ? json.toolCalls.map((tc: any) => ({
                  id: tc.id,
                  name: tc.function?.name || tc.name,
                  arguments: tc.function?.arguments || tc.arguments,
                  status: tc.status || 'completed',
                  mcpId: tc.mcpId || (tc.function?.name || tc.name || '').split('/')[0],
                  result: tc.result,
                })) : undefined;
              }
            } catch {
              // Not JSON, use as-is (final message)
            }
            
            // Only include toolCalls if message content is JSON with toolCalls (intermediate message)
            // Don't inherit toolCalls from tempMsg for final message
            const realMessage = {
              ...added.message,
              content: parsedContent,
              toolCalls,
              thinkingContent: added.message.thinkingContent || tempMsg.thinkingContent,
            } as any;
            
            // Remove temp message and insert real message at correct position
            const updated = list.filter((m) => m.id !== tempId);
            const newMsgId = getNumericId(realMessage.id);
            const insertIndex = updated.findIndex((m) => getNumericId(m.id) > newMsgId);
            
            if (insertIndex === -1) {
              updated.push(realMessage);
            } else {
              updated.splice(insertIndex, 0, realMessage);
            }
            
            streamingMessageId.set(null);
            return updated;
          } else {
            streamingMessageId.set(null);
          }
        }
        
        // Parse JSON content for assistant messages to extract toolCalls
        let parsedContent = added.message.content;
        let toolCalls: any[] | undefined;
        
        if (added.message.role === 'assistant') {
          try {
            const json = JSON.parse(added.message.content);
            if (json.content !== undefined && json.toolCalls) {
              parsedContent = json.content;
              toolCalls = json.toolCalls.map((tc: any) => ({
                id: tc.id,
                name: tc.function?.name || tc.name,
                arguments: tc.function?.arguments || tc.arguments,
                status: tc.status || 'completed',
                mcpId: tc.mcpId || (tc.function?.name || tc.name || '').split('/')[0],
                result: tc.result,
              }));
            }
          } catch {
            // Not JSON, use as-is
          }
        }
        
        const messageToAdd = {
          ...added.message,
          content: parsedContent,
          toolCalls,
        } as any;
        
        // Insert message in correct position based on ID
        const newMsgId = getNumericId(messageToAdd.id);
        const insertIndex = list.findIndex((m) => getNumericId(m.id) > newMsgId);
        
        if (insertIndex === -1) {
          return [...list, messageToAdd];
        } else {
          const updated = [...list];
          updated.splice(insertIndex, 0, messageToAdd);
          return updated;
        }
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
    case 'chatToolCallStarted': {
      const { toolCallId, toolName, arguments: args, mcpId } = data as {
        chatId: number;
        toolCallId: string;
        toolName: string;
        arguments: string;
        mcpId: string;
      };
      const newToolCall = {
        id: toolCallId,
        name: toolName,
        arguments: args,
        status: 'running' as const,
        mcpId,
      };
      pendingToolCalls.update((list) => [...list, newToolCall]);
      
      let tempId = get(streamingMessageId);
      
      if (!tempId) {
        const chatId = get(currentChatId);
        if (!chatId) break;
        
        tempId = `temp-${Date.now()}`;
        const assistantMessage = {
          id: tempId,
          chatId,
          role: 'assistant' as const,
          content: '',
          createdAt: new Date().toISOString(),
          model: get(selectedModel) || '',
          thinkingContent: get(streamingThinkingContent) || undefined,
          toolCalls: [newToolCall],
        };
        messages.update((list) => [...list, assistantMessage as any]);
        streamingMessageId.set(tempId);
      } else {
        messages.update((list) =>
          list.map((m) => {
            if (m.id === tempId) {
              const existingToolCalls = m.toolCalls || [];
              // Prevent duplicate tool calls
              if (existingToolCalls.some((tc) => tc.id === toolCallId)) {
                return m;
              }
              return { ...m, toolCalls: [...existingToolCalls, newToolCall] };
            }
            return m;
          })
        );
      }
      break;
    }
    case 'chatToolCallCompleted': {
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
      
      // Search for the assistant message with the matching toolCallId
      // (streamingMessageId may be null if MessageAdded already arrived)
      messages.update((list) => {
        let updatedMessageId: number | string | null = null;
        const updated = list.map((m) => {
          if (m.role === 'assistant' && m.toolCalls) {
            const toolCallIndex = m.toolCalls.findIndex((tc) => tc.id === toolCallId);
            if (toolCallIndex !== -1) {
              updatedMessageId = m.id;
              const updatedToolCalls = [...m.toolCalls];
              updatedToolCalls[toolCallIndex] = {
                ...updatedToolCalls[toolCallIndex],
                result,
                status: 'completed' as const,
              };
              return { ...m, toolCalls: updatedToolCalls };
            }
          }
          return m;
        });
        return updated;
      });
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

// Test-only exports for state-based testing
export {
  loadChats as _test_loadChats,
  loadAvailableModels as _test_loadAvailableModels,
  createChat as _test_createChat,
  selectChat as _test_selectChat,
  deleteChat as _test_deleteChat,
  deleteAllChats as _test_deleteAllChats,
  sendMessage as _test_sendMessage,
  editMessage as _test_editMessage,
  abortChat as _test_abortChat,
  pauseChat as _test_pauseChat,
  resumeChat as _test_resumeChat,
  handleChatEvent as _test_handleChatEvent,
};
