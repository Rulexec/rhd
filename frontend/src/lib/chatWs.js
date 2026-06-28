import { sendRequest, generateRequestId } from './ws.js';
import {
  chats,
  currentChatId,
  messages,
  streamingContent,
  isStreaming,
  streamError,
} from './chatStores.js';

export async function loadChats() {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'listChats', id });
  if (response.success) {
    chats.set(response.data || []);
  }
  return response;
}

export async function createChat(title) {
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

export async function selectChat(chatId) {
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

export async function deleteChat(chatId) {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'deleteChat', id, chatId });
  if (response.success) {
    chats.update((list) => list.filter((c) => c.id !== chatId));
    const currentId = currentChatId.get();
    if (currentId === chatId) {
      currentChatId.set(null);
      messages.set([]);
    }
  }
  return response;
}

export async function sendMessage(content, model) {
  const chatId = currentChatId.get();
  if (!chatId) return;

  const tempId = Date.now();
  const userMessage = {
    id: tempId,
    chatId,
    role: 'user',
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

export async function editMessage(messageId, newContent, model) {
  const chatId = currentChatId.get();
  if (!chatId) return;

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

export async function abortChat() {
  const chatId = currentChatId.get();
  if (!chatId) return;

  const id = generateRequestId();
  const response = await sendRequest({ type: 'abortChat', id, chatId });
  return response;
}

export function handleChatEvent(event, data) {
  switch (event) {
    case 'chatStreamChunk':
      streamingContent.update((c) => c + data.content);
      break;
    case 'chatStreamFinished':
      messages.update((list) => [
        ...list,
        {
          id: data.messageId,
          chatId: data.chatId,
          role: 'assistant',
          content: streamingContent.get(),
          createdAt: new Date().toISOString(),
        },
      ]);
      isStreaming.set(false);
      streamingContent.set('');
      streamError.set(null);
      break;
    case 'chatStreamError':
      streamError.set(data.error);
      isStreaming.set(false);
      break;
    case 'chatMessageAdded':
      messages.update((list) => {
        if (list.some((m) => m.id === data.message.id)) {
          return list;
        }
        return [...list, data.message];
      });
      break;
    case 'chatUpdated':
      chats.update((list) =>
        list.map((c) => (c.id === data.chatId ? { ...c, title: data.title } : c))
      );
      break;
  }
}
