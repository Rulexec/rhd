import { get } from 'svelte/store';
import {
  chats,
  currentChatId,
  messages,
  streamingThinkingContent,
  isStreaming,
  streamError,
  selectedModel,
  streamingMessageId,
  isPaused,
  isAborted,
  pendingToolCalls,
  availableRoles,
  activeRole,
  todoList,
  pendingMessages,
} from '@/lib/chatStores';
import {
  handleMcpStatusEvent,
  handleProjectAttachedEvent,
  handleProjectDetachedEvent,
} from '@/lib/projectStores';
import { parseAssistantMessage, parseToolResult } from './parsers';

function promotePendingMessagesToChat(): void {
  const chatId = get(currentChatId);
  if (!chatId) return;

  const queued = get(pendingMessages);
  if (queued.length === 0) return;

  messages.update((list) => {
    const highestNumericId = list.reduce(
      (highest, m) => (typeof m.id === 'number' && m.id > highest ? m.id : highest),
      0
    );
    // A numeric Date.now()-style id lets a later chatMessageAdded echo replace the
    // promoted message in place instead of rendering a second copy. Staying above
    // every existing id keeps keys unique in the rendered list.
    let promotedId = Math.max(Date.now(), highestNumericId + 1);
    const promoted = queued.map((entry) => ({
      id: promotedId++,
      chatId,
      role: 'user' as const,
      content: entry.content,
      createdAt: entry.queuedAt,
      model: entry.model,
    }));
    return [...list, ...(promoted as any[])];
  });
  pendingMessages.set([]);
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
      streamingMessageId.set(null);
      streamingThinkingContent.set('');
      // The interrupted call has ended. While the chat is resumed the daemon now
      // drains the message queue, so queued messages are no longer waiting to be sent
      if (!get(isPaused) && !get(isAborted)) {
        promotePendingMessagesToChat();
      }
      break;
    }
    case 'chatStreamError': {
      const error = data as { error: string };
      streamError.set(error.error);
      isStreaming.set(false);
      streamingMessageId.set(null);
      streamingThinkingContent.set('');
      break;
    }
    case 'chatMessageAdded': {
      const added = data as { message: { id: number; role: string; content: string; thinkingContent?: string } };
      const tempId = get(streamingMessageId);
      if (added.message.role === 'user') {
        // The daemon confirmed this message, so drop the optimistic gray copy
        pendingMessages.update((list) => {
          const pendingIdx = list.findIndex((m) => m.content === added.message.content);
          if (pendingIdx === -1) return list;
          return list.filter((_, idx) => idx !== pendingIdx);
        });
      }
      messages.update((list) => {
        const getNumericId = (id: number | string): number => {
          if (typeof id === 'number') return id;
          if (typeof id === 'string' && id.startsWith('temp-')) {
            return Number.MAX_SAFE_INTEGER;
          }
          return parseInt(id as string, 10) || Number.MAX_SAFE_INTEGER;
        };

        if (list.some((m) => m.id === added.message.id)) {
          return list;
        }

        if (added.message.role === 'user') {
          const tempUserIdx = list.findIndex(
            (m) => m.role === 'user' && m.content === added.message.content && typeof m.id === 'number' && m.id > 1000000000000
          );
          if (tempUserIdx !== -1) {
            const updated = [...list];
            updated[tempUserIdx] = added.message as any;
            return updated;
          }
          return [...list, added.message as any];
        }

        if (added.message.role === 'tool') {
          const parsed = parseToolResult(added.message.content);
          if (parsed) {
            const toolCallId = parsed.toolCallId;
            const result = parsed.result;
            const isError = parsed.isError;
            const toolStatus = isError ? 'failed' as const : 'completed' as const;
            
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
                    status: toolStatus,
                  };
                  return { ...m, toolCalls: updatedToolCalls };
                }
              }
              return m;
            });
            return updated;
          } else {
            return [...list, added.message as any];
          }
        }

        if (added.message.role === 'assistant' && tempId) {
          const tempIdx = list.findIndex((m) => m.id === tempId);
          if (tempIdx !== -1) {
            const tempMsg = list[tempIdx];
            
            const parsed = parseAssistantMessage(added.message);
            const parsedContent = parsed.content;
            const toolCalls = parsed.toolCalls;
            
            const realMessage = {
              ...added.message,
              content: parsedContent,
              toolCalls,
              thinkingContent: added.message.thinkingContent || tempMsg.thinkingContent,
            } as any;
            
            const updated = list.filter((m) => m.id !== tempId);
            const newMsgId = getNumericId(realMessage.id);
            const insertIndex = updated.findIndex((m) => getNumericId(m.id) > newMsgId);
            
            if (insertIndex === -1) {
              updated.push(realMessage);
            } else {
              updated.splice(insertIndex, 0, realMessage);
            }
            
            streamingMessageId.set(null);
            streamingThinkingContent.set('');
            return updated;
          } else {
            streamingMessageId.set(null);
          }
        }
        
        const parsed = parseAssistantMessage(added.message);
        const parsedContent = parsed.content;
        const toolCalls = parsed.toolCalls;
        
        const messageToAdd = {
          ...added.message,
          content: parsedContent,
          toolCalls,
        } as any;
        
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
      const { toolCallId, result, isError } = data as {
        chatId: number;
        toolCallId: string;
        result: string;
        isError: boolean;
      };
      const toolStatus = isError ? 'failed' as const : 'completed' as const;
      pendingToolCalls.update((list) =>
        list.map((tc) =>
          tc.id === toolCallId ? { ...tc, result, status: toolStatus } : tc
        )
      );
      
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
                status: toolStatus,
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
      isAborted.set(false);
      isStreaming.set(false);
      break;
    }
    case 'chatResumed': {
      isPaused.set(false);
      isAborted.set(false);
      isStreaming.set(true);
      break;
    }
    case 'streamAborted': {
      const tempId = get(streamingMessageId);
      if (tempId) {
        // Only remove the message if it doesn't have tool calls
        // (messages with tool calls should be preserved even on abort)
        messages.update((list) => {
          const msg = list.find(m => m.id === tempId);
          if (msg && msg.toolCalls && msg.toolCalls.length > 0) {
            // Keep the message if it has tool calls
            return list;
          }
          // Otherwise, remove it
          return list.filter((m) => m.id !== tempId);
        });
        streamingMessageId.set(null);
      }
      
      isPaused.set(true);
      isAborted.set(true);
      isStreaming.set(false);
      streamingThinkingContent.set('');
      break;
    }
    case 'roleChanged': {
      const { projectName, roleName } = data as {
        chatId: number;
        projectName: string;
        roleName: string;
      };
      activeRole.set({ projectName, roleName });
      break;
    }
    case 'rolesUpdated': {
      const { roles, activeRoleProject, activeRoleName } = data as {
        chatId: number;
        roles: import('../types/index').RoleInfo[];
        activeRoleProject?: string;
        activeRoleName?: string;
      };
      availableRoles.set(roles);
      if (activeRoleProject && activeRoleName) {
        activeRole.set({
          projectName: activeRoleProject,
          roleName: activeRoleName,
        });
      } else {
        activeRole.set(null);
      }
      break;
    }
    case 'activeRoleCleared': {
      activeRole.set(null);
      break;
    }
    case 'todoListUpdated': {
      const { items } = data as {
        chatId: number;
        items: import('../types/index').TodoItem[];
      };
      todoList.set(items);
      break;
    }
  }
}
