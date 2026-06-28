import { get } from 'svelte/store';
import { sendRequest, generateRequestId } from './ws';
import type { WsResponse } from './types/ws';
import {
  chats,
  currentChatId,
  messages,
  streamingContent,
  isStreaming,
  streamError,
} from './chatStores';
import type { WsEvent } from './types/ws';

export async function loadChats(): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'listChats', id });
  if (response.success) {
    chats.set(response.data || []);
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
    };
    chats.update((list) => [...list, newChat]);
    currentChatId.set(chatId);
    messages.set([]);
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
  };
  messages.update((list) => [...list, userMessage]);

  isStreaming.set(true);
  streamingContent.set('');
  streamError.set(null);

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
  streamError.set(null);

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

export function handleChatEvent(event: string, data: unknown): void {
  switch (event) {
    case 'chatStreamChunk': {
      const chunk = data as { content: string };
      streamingContent.update((c) => c + chunk.content);
      break;
    }
    case 'chatStreamFinished': {
      const finished = data as { messageId: number; chatId: number };
      messages.update((list) => [
        ...list,
        {
          id: finished.messageId,
          chatId: finished.chatId,
          role: 'assistant' as const,
          content: get(streamingContent),
          createdAt: new Date().toISOString(),
        },
      ]);
      isStreaming.set(false);
      streamingContent.set('');
      streamError.set(null);
      break;
    }
    case 'chatStreamError': {
      const error = data as { error: string };
      streamError.set(error.error);
      isStreaming.set(false);
      break;
    }
    case 'chatMessageAdded': {
      const added = data as { message: { id: number } };
      messages.update((list) => {
        if (list.some((m) => m.id === added.message.id)) {
          return list;
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
  }
}
