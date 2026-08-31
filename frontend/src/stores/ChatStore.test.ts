import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatStore } from './ChatStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat, Message, StreamChunkData, StreamFinishedData } from '../lib/api/schemas.js';
import type { ChatEventHandlers, QueueMessageEventHandlers, StreamEventHandlers } from '../lib/api/chatApiImpl.js';
import * as chatApiImpl from '../lib/api/chatApiImpl.js';

/**
 * Unit tests for ChatStore.
 *
 * Async methods are generator prototype methods that makeAutoObservable
 * auto-wraps into flows, so calling e.g. `store.selectChat(1)` or
 * `store.clearChat()` returns a CancellablePromise directly and can be
 * awaited (see mobx-probe.test.ts for the underlying MobX semantics).
 *
 * Event payloads use the real WebSocket data shapes from schemas.ts:
 * - message events: { chatId, message, chatVersion }
 * - message deleted events: { chatId, messageId, chatVersion }
 */
describe('ChatStore', () => {
  let store: ChatStore;
  let mockChatApi: ChatApi;

  const mockChat: Chat = {
    id: 1,
    title: 'Test Chat',
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    tags: [],
    version: 1
  };

  const mockMessage: Message = {
    id: 1,
    chatId: 1,
    role: 'user',
    content: 'Hello',
    createdAt: '2024-01-01T00:00:00Z',
    tags: [],
    isFinished: true,
    isStreaming: false,
    toolCalls: []
  };

  beforeEach(() => {
    mockChatApi = {
      subscribeChat: vi.fn().mockResolvedValue(undefined),
      unsubscribeChat: vi.fn().mockResolvedValue(undefined),
      getChat: vi.fn().mockResolvedValue({ chat: mockChat, messages: [mockMessage] }),
      getQueueMessages: vi.fn().mockResolvedValue({ messages: [] }),
      getTools: vi.fn().mockResolvedValue({ tools: [] }),
      onChatEvents: vi.fn().mockReturnValue(() => {}),
      onQueueMessageEvents: vi.fn().mockReturnValue(() => {}),
      // Other ChatApi members are not exercised by these tests.
      subscribeChatsList: vi.fn(),
      unsubscribeChatsList: vi.fn(),
      listChats: vi.fn(),
      createChat: vi.fn(),
      deleteChat: vi.fn(),
      generateChatTitle: vi.fn(),
      onChatListEvents: vi.fn(),
      addQueueMessage: vi.fn(),
      subscribePluginsList: vi.fn(),
      getPlugins: vi.fn(),
      onPluginListEvents: vi.fn()
    } as unknown as ChatApi;

    store = new ChatStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.currentChatId).toBe(null);
    expect(store.currentChat).toBe(null);
    expect(store.messages).toEqual([]);
    expect(store.queueMessages).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
    expect(store.allMessages).toEqual([]);
  });

  it('should select chat and load messages', async () => {
    await store.selectChat(1);

    expect(mockChatApi.subscribeChat).toHaveBeenCalledWith(1);
    expect(mockChatApi.getChat).toHaveBeenCalledWith(1);
    expect(mockChatApi.getQueueMessages).toHaveBeenCalledWith(1);
    expect(store.currentChatId).toBe(1);
    expect(store.currentChat).toEqual(mockChat);
    expect(store.messages).toEqual([mockMessage]);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should return a thenable from selectChat (auto-wrapped flow)', async () => {
    const result = store.selectChat(1);

    expect(typeof (result as { then?: unknown }).then).toBe('function');
    await result;
    expect(store.currentChatId).toBe(1);
  });

  it('should handle select error', async () => {
    vi.mocked(mockChatApi.getChat).mockRejectedValueOnce(new Error('Load failed'));

    await store.selectChat(1);

    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should clear chat and reset state', async () => {
    await store.selectChat(1);
    await store.clearChat();

    expect(mockChatApi.unsubscribeChat).toHaveBeenCalledWith(1);
    expect(store.currentChatId).toBe(null);
    expect(store.currentChat).toBe(null);
    expect(store.messages).toEqual([]);
    expect(store.queueMessages).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should combine regular and queue messages in allMessages', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2, content: 'Queue msg' };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });

    await store.selectChat(1);

    expect(store.allMessages.length).toBe(2);
    expect(store.allMessages[0]!.isQueue).toBe(false);
    expect(store.allMessages[1]!.isQueue).toBe(true);
  });

  it('should sort allMessages by createdAt ASC', async () => {
    const msg2: Message = { ...mockMessage, id: 2, createdAt: '2024-01-01T00:01:00Z' };
    const msg1: Message = { ...mockMessage, id: 1, createdAt: '2024-01-01T00:00:00Z' };
    vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({ chat: mockChat, messages: [msg2, msg1] });

    await store.selectChat(1);

    expect(store.allMessages[0]!.id).toBe(1); // Older first
    expect(store.allMessages[1]!.id).toBe(2);
  });

  it('should handle message added event', async () => {
    let onMessageAddedHandler: ChatEventHandlers['onMessageAdded'];
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((_chatId, handlers) => {
      onMessageAddedHandler = handlers.onMessageAdded;
      return () => {};
    });

    await store.selectChat(1);

    const newMessage: Message = { ...mockMessage, id: 3, content: 'New message' };
    onMessageAddedHandler!({ chatId: 1, message: newMessage, chatVersion: 1 });

    expect(store.messages.some(m => m.id === 3)).toBe(true);
    expect(store.messages[1]!.content).toBe('New message');
  });

  it('should remove message from queue when added as regular message', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2, content: 'Queued' };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });

    let onMessageAddedHandler: ChatEventHandlers['onMessageAdded'];
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((_chatId, handlers) => {
      onMessageAddedHandler = handlers.onMessageAdded;
      return () => {};
    });

    await store.selectChat(1);
    expect(store.queueMessages.length).toBe(1);

    // Message added as regular message (queue → regular promotion)
    onMessageAddedHandler!({ chatId: 1, message: queueMessage, chatVersion: 1 });

    expect(store.queueMessages.length).toBe(0);
    expect(store.messages.some(m => m.id === 2)).toBe(true);
  });

  it('should handle message updated event', async () => {
    let onMessageUpdatedHandler: ChatEventHandlers['onMessageUpdated'];
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((_chatId, handlers) => {
      onMessageUpdatedHandler = handlers.onMessageUpdated;
      return () => {};
    });

    await store.selectChat(1);

    const updatedMessage: Message = { ...mockMessage, content: 'Updated' };
    onMessageUpdatedHandler!({ chatId: 1, message: updatedMessage, chatVersion: 1 });

    expect(store.messages[0]!.content).toBe('Updated');
  });

  it('should handle message deleted event', async () => {
    let onMessageDeletedHandler: ChatEventHandlers['onMessageDeleted'];
    vi.mocked(mockChatApi.onChatEvents).mockImplementation((_chatId, handlers) => {
      onMessageDeletedHandler = handlers.onMessageDeleted;
      return () => {};
    });

    await store.selectChat(1);
    expect(store.messages.length).toBe(1);

    onMessageDeletedHandler!({ chatId: 1, messageId: 1, chatVersion: 1 });

    expect(store.messages.length).toBe(0);
  });

  it('should handle queue message added event', async () => {
    let onQueueMessageAddedHandler: QueueMessageEventHandlers['onQueueMessageAdded'];
    vi.mocked(mockChatApi.onQueueMessageEvents).mockImplementation((_chatId, handlers) => {
      onQueueMessageAddedHandler = handlers.onQueueMessageAdded;
      return () => {};
    });

    await store.selectChat(1);

    const queueMessage: Message = { ...mockMessage, id: 3 };
    onQueueMessageAddedHandler!({ chatId: 1, message: queueMessage, chatVersion: 1 });

    expect(store.queueMessages.some(m => m.id === 3)).toBe(true);
    expect(store.queueMessages[0]!.id).toBe(3);
  });

  it('should handle queue message updated event', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2, content: 'Queued' };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });

    let onQueueMessageUpdatedHandler: QueueMessageEventHandlers['onQueueMessageUpdated'];
    vi.mocked(mockChatApi.onQueueMessageEvents).mockImplementation((_chatId, handlers) => {
      onQueueMessageUpdatedHandler = handlers.onQueueMessageUpdated;
      return () => {};
    });

    await store.selectChat(1);

    const updatedQueueMessage: Message = { ...queueMessage, content: 'Updated queued' };
    onQueueMessageUpdatedHandler!({ chatId: 1, message: updatedQueueMessage, chatVersion: 1 });

    expect(store.queueMessages[0]!.content).toBe('Updated queued');
  });

  it('should handle queue message deleted event', async () => {
    const queueMessage: Message = { ...mockMessage, id: 2 };
    vi.mocked(mockChatApi.getQueueMessages).mockResolvedValueOnce({ messages: [queueMessage] });

    let onQueueMessageDeletedHandler: QueueMessageEventHandlers['onQueueMessageDeleted'];
    vi.mocked(mockChatApi.onQueueMessageEvents).mockImplementation((_chatId, handlers) => {
      onQueueMessageDeletedHandler = handlers.onQueueMessageDeleted;
      return () => {};
    });

    await store.selectChat(1);
    expect(store.queueMessages.length).toBe(1);

    onQueueMessageDeletedHandler!({ chatId: 1, messageId: 2, chatVersion: 1 });

    expect(store.queueMessages.length).toBe(0);
  });

  it('should cleanup event listeners on clear', async () => {
    const cleanupEvents = vi.fn();
    const cleanupQueueEvents = vi.fn();

    vi.mocked(mockChatApi.onChatEvents).mockReturnValue(cleanupEvents);
    vi.mocked(mockChatApi.onQueueMessageEvents).mockReturnValue(cleanupQueueEvents);

    await store.selectChat(1);
    await store.clearChat();

    expect(cleanupEvents).toHaveBeenCalled();
    expect(cleanupQueueEvents).toHaveBeenCalled();
  });
  
  describe('ChatStore streaming', () => {
    let store: ChatStore;
    let mockChatApi: ChatApi;
  
    const mockChat: Chat = {
      id: 1,
      title: 'Test Chat',
      createdAt: '2024-01-01T00:00:00Z',
      updatedAt: '2024-01-01T00:00:00Z',
      tags: [],
      version: 1
    };
  
    const mockMessage: Message = {
      id: 1,
      chatId: 1,
      role: 'assistant',
      content: '',
      createdAt: '2024-01-01T00:00:00Z',
      tags: [],
      isFinished: false,
      isStreaming: false,
      toolCalls: []
    };
  
    const streamingMessage: Message = {
      ...mockMessage,
      id: 2,
      isStreaming: true
    };
  
    beforeEach(() => {
      vi.clearAllMocks();
  
      mockChatApi = {
        subscribeChat: vi.fn().mockResolvedValue(undefined),
        unsubscribeChat: vi.fn().mockResolvedValue(undefined),
        getChat: vi.fn().mockResolvedValue({ chat: mockChat, messages: [mockMessage] }),
        getQueueMessages: vi.fn().mockResolvedValue({ messages: [] }),
        onChatEvents: vi.fn().mockReturnValue(() => {}),
        onQueueMessageEvents: vi.fn().mockReturnValue(() => {}),
        subscribeChatsList: vi.fn(),
        unsubscribeChatsList: vi.fn(),
        listChats: vi.fn(),
        createChat: vi.fn(),
        deleteChat: vi.fn(),
        generateChatTitle: vi.fn(),
        onChatListEvents: vi.fn(),
        addQueueMessage: vi.fn(),
        subscribePluginsList: vi.fn(),
        getPlugins: vi.fn(),
        onPluginListEvents: vi.fn()
      } as unknown as ChatApi;
  
      store = new ChatStore({ chatApi: mockChatApi });
    });
  
    describe('streamingMessage getter', () => {
      it('should return null when no messages are streaming', async () => {
        await store.selectChat(1);
        expect(store.streamingMessage).toBe(null);
      });
  
      it('should return first streaming message when one exists', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [mockMessage, streamingMessage]
        });
  
        await store.selectChat(1);
        expect(store.streamingMessage).toEqual(streamingMessage);
      });
  
      it('should return first streaming message when multiple exist', async () => {
        const anotherStreaming: Message = { ...streamingMessage, id: 3 };
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [mockMessage, streamingMessage, anotherStreaming]
        });
  
        await store.selectChat(1);
        expect(store.streamingMessage).toEqual(streamingMessage);
      });
    });
  
    describe('streamContent getter', () => {
      it('should return null when streaming is finished and no content', () => {
        expect(store.streamContent).toBe(null);
      });
  
      it('should return streaming content object when streaming is active', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: 'Hello',
          reasoningContent: 'Thinking...',
          toolCalls: [],
          isFinished: false
        });
  
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockReturnValue(() => {});
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(store.streamContent).toEqual({
          content: 'Hello',
          reasoningContent: 'Thinking...',
          toolCalls: [],
          isFinished: false
        });
  
        dispose();
      });
    });
  
    describe('init() method', () => {
      it('should return a disposer function', () => {
        const dispose = store.init();
        expect(typeof dispose).toBe('function');
        dispose();
      });
  
      it('should call streamSubscribe when streaming message appears', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        const streamSubscribeSpy = vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: '',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockReturnValue(() => {});
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(streamSubscribeSpy).toHaveBeenCalledWith(1);
  
        dispose();
      });
  
      it('should call onStreamEvents to register event listeners', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: '',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        const onStreamEventsSpy = vi.spyOn(chatApiImpl, 'onStreamEvents').mockReturnValue(() => {});
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(onStreamEventsSpy).toHaveBeenCalledWith(1, expect.any(Object));
  
        dispose();
      });
  
      it('should cleanup streaming when disposer is called', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: 'Test',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        const cleanupFn = vi.fn();
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockReturnValue(cleanupFn);
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(store.streamingContent).toBe('Test');
  
        dispose();
  
        expect(cleanupFn).toHaveBeenCalled();
        expect(store.streamingContent).toBe('');
        expect(store.streamingReasoningContent).toBe('');
        expect(store.streamingToolCalls).toEqual([]);
        expect(store.streamingIsFinished).toBe(true);
      });
    });
  
    describe('stream event handling', () => {
      it('should handle contentDelta stream chunks', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: '',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        let streamHandlers: StreamEventHandlers = {};
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockImplementation((_chatId, handlers) => {
          streamHandlers = handlers;
          return () => {};
        });
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        // Simulate stream chunk
        const chunk: StreamChunkData = {
          chatId: 1,
          type: 'contentDelta',
          content: 'Hello'
        };
        streamHandlers.onStreamChunk!(chunk);
  
        expect(store.streamingContent).toBe('Hello');
  
        // Simulate another chunk
        const chunk2: StreamChunkData = {
          chatId: 1,
          type: 'contentDelta',
          content: ' World'
        };
        streamHandlers.onStreamChunk!(chunk2);
  
        expect(store.streamingContent).toBe('Hello World');
  
        dispose();
      });
  
      it('should handle reasoningDelta stream chunks', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: '',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        let streamHandlers: StreamEventHandlers = {};
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockImplementation((_chatId, handlers) => {
          streamHandlers = handlers;
          return () => {};
        });
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        // Simulate reasoning chunk
        const chunk: StreamChunkData = {
          chatId: 1,
          type: 'reasoningDelta',
          content: 'Thinking...'
        };
        streamHandlers.onStreamChunk!(chunk);
  
        expect(store.streamingReasoningContent).toBe('Thinking...');
  
        dispose();
      });
  
      it('should handle toolCallDelta stream chunks', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: '',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        let streamHandlers: StreamEventHandlers = {};
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockImplementation((_chatId, handlers) => {
          streamHandlers = handlers;
          return () => {};
        });
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        // Simulate tool call chunk
        const chunk: StreamChunkData = {
          chatId: 1,
          type: 'toolCallDelta',
          toolCalls: [{ id: '1', name: 'testTool', arguments: '{"arg":' }]
        };
        streamHandlers.onStreamChunk!(chunk);
  
        expect(store.streamingToolCalls).toHaveLength(1);
        expect(store.streamingToolCalls[0]).toEqual({ id: '1', name: 'testTool', arguments: '{"arg":' });
  
        // Simulate continuation of same tool call
        const chunk2: StreamChunkData = {
          chatId: 1,
          type: 'toolCallDelta',
          toolCalls: [{ id: '1', name: 'testTool', arguments: '"value"}' }]
        };
        streamHandlers.onStreamChunk!(chunk2);
  
        expect(store.streamingToolCalls).toHaveLength(1);
        expect(store.streamingToolCalls[0]!.arguments).toBe('{"arg":"value"}');
  
        dispose();
      });
  
      it('should handle stream finished event', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: 'Test',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        let streamHandlers: StreamEventHandlers = {};
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockImplementation((_chatId, handlers) => {
          streamHandlers = handlers;
          return () => {};
        });
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(store.streamingIsFinished).toBe(false);
  
        // Simulate stream finished
        const finished: StreamFinishedData = { chatId: 1 };
        streamHandlers.onStreamFinished!(finished);
  
        expect(store.streamingIsFinished).toBe(true);
  
        dispose();
      });
    });
  
    describe('streaming lifecycle', () => {
      it('should stop streaming when streaming message is removed', async () => {
        vi.mocked(mockChatApi.getChat).mockResolvedValueOnce({
          chat: mockChat,
          messages: [streamingMessage]
        });
  
        vi.spyOn(chatApiImpl, 'streamSubscribe').mockResolvedValueOnce({
          content: 'Test',
          reasoningContent: '',
          toolCalls: [],
          isFinished: false
        });
  
        const cleanupFn = vi.fn();
        vi.spyOn(chatApiImpl, 'onStreamEvents').mockReturnValue(cleanupFn);
  
        const dispose = store.init();
        await store.selectChat(1);
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(store.streamingContent).toBe('Test');
  
        // Simulate message update that removes streaming flag
        let onMessageUpdatedHandler: ChatEventHandlers['onMessageUpdated'];
        vi.mocked(mockChatApi.onChatEvents).mockImplementation((_chatId, handlers) => {
          onMessageUpdatedHandler = handlers.onMessageUpdated;
          return () => {};
        });
  
        await store.selectChat(1);
  
        const finishedMessage: Message = { ...streamingMessage, isStreaming: false, isFinished: true };
        onMessageUpdatedHandler!({ chatId: 1, message: finishedMessage, chatVersion: 2 });
  
        // Wait for reaction to fire
        await new Promise(resolve => setTimeout(resolve, 0));
  
        expect(cleanupFn).toHaveBeenCalled();
        expect(store.streamingContent).toBe('');
        expect(store.streamingIsFinished).toBe(true);
  
        dispose();
      });
    });
  });
});