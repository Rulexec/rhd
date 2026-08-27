import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ChatsListStore } from './ChatsListStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { Chat } from '../lib/api/schemas.js';
import type { ChatListEventHandlers } from '../lib/api/chatApiImpl.js';

/**
 * Unit tests for ChatsListStore.
 *
 * Async methods are generator prototype methods that makeAutoObservable
 * auto-wraps into flows, so calling e.g. `store.loadChats()` returns a
 * CancellablePromise directly and can be awaited (see mobx-probe.test.ts
 * for the underlying MobX semantics).
 */
describe('ChatsListStore', () => {
  let store: ChatsListStore;
  let mockChatApi: ChatApi;

  const mockChat: Chat = {
    id: 1,
    title: 'Test Chat',
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    tags: [],
    version: 1
  };

  beforeEach(() => {
    mockChatApi = {
      subscribeChatsList: vi.fn().mockResolvedValue(undefined),
      unsubscribeChatsList: vi.fn().mockResolvedValue(undefined),
      listChats: vi.fn().mockResolvedValue({ chats: [mockChat] }),
      createChat: vi.fn().mockResolvedValue({ chatId: 2 }),
      deleteChat: vi.fn().mockResolvedValue(undefined),
      generateChatTitle: vi.fn().mockReturnValue('2024-01-01 00:00'),
      onChatListEvents: vi.fn().mockReturnValue(() => {}),
      // Other ChatApi members are not exercised by these tests.
      subscribeChat: vi.fn(),
      unsubscribeChat: vi.fn(),
      getChat: vi.fn(),
      onChatEvents: vi.fn(),
      getQueueMessages: vi.fn(),
      addQueueMessage: vi.fn(),
      onQueueMessageEvents: vi.fn(),
      subscribePluginsList: vi.fn(),
      getPlugins: vi.fn(),
      onPluginListEvents: vi.fn()
    } as unknown as ChatApi;

    store = new ChatsListStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.chats).toEqual([]);
    expect(store.hasChats).toBe(false);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should load chats', async () => {
    await store.loadChats();

    expect(mockChatApi.listChats).toHaveBeenCalled();
    expect(store.chats).toEqual([mockChat]);
    expect(store.hasChats).toBe(true);
    expect(store.loading).toBe(false);
  });

  it('should handle load error', async () => {
    vi.mocked(mockChatApi.listChats).mockRejectedValueOnce(new Error('Load failed'));

    await store.loadChats();

    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should create new chat', async () => {
    const chatId = await store.createNewChat();

    expect(mockChatApi.generateChatTitle).toHaveBeenCalled();
    expect(mockChatApi.createChat).toHaveBeenCalled();
    expect(chatId).toBe(2);
  });

  it('should return null when create chat fails', async () => {
    vi.mocked(mockChatApi.createChat).mockRejectedValueOnce(new Error('Create failed'));

    const chatId = await store.createNewChat();

    expect(chatId).toBe(null);
    expect(store.error).toBe('Create failed');
  });

  it('should delete all chats', async () => {
    await store.loadChats();
    const success = await store.deleteAllChats();

    expect(mockChatApi.deleteChat).toHaveBeenCalledWith(1);
    expect(success).toBe(true);
  });

  it('should sort chats by updatedAt DESC', async () => {
    const chat1: Chat = { ...mockChat, id: 1, updatedAt: '2024-01-01T00:00:00Z' };
    const chat2: Chat = { ...mockChat, id: 2, updatedAt: '2024-01-02T00:00:00Z' };

    vi.mocked(mockChatApi.listChats).mockResolvedValueOnce({ chats: [chat1, chat2] });
    await store.loadChats();

    expect(store.chats[0]!.id).toBe(2); // Newer first
    expect(store.chats[1]!.id).toBe(1);
  });

  it('should handle chat created event', async () => {
    let onChatCreatedHandler: ChatListEventHandlers['onChatCreated'];
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatCreatedHandler = handlers.onChatCreated;
      return () => {};
    });

    await store.init();

    const newChat: Chat = { ...mockChat, id: 3 };
    onChatCreatedHandler!({ chat: newChat, chatVersion: 1 });

    expect(store.chats.some(c => c.id === 3)).toBe(true);
  });

  it('should not duplicate a chat on created event for an existing id', async () => {
    let onChatCreatedHandler: ChatListEventHandlers['onChatCreated'];
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatCreatedHandler = handlers.onChatCreated;
      return () => {};
    });

    await store.init();

    const duplicate: Chat = { ...mockChat, title: 'Duplicated Title' };
    onChatCreatedHandler!({ chat: duplicate, chatVersion: 1 });

    expect(store.chats).toHaveLength(1);
    expect(store.chats[0]!.title).toBe('Duplicated Title');
  });

  it('should handle chat updated event', async () => {
    let onChatUpdatedHandler: ChatListEventHandlers['onChatUpdated'];
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatUpdatedHandler = handlers.onChatUpdated;
      return () => {};
    });

    // init() registers the event listeners; the store loads the initial list.
    await store.init();

    const updatedChat: Chat = { ...mockChat, title: 'Updated Title' };
    onChatUpdatedHandler!({ chat: updatedChat, chatVersion: 1 });

    expect(store.chats[0]!.title).toBe('Updated Title');
  });

  it('should handle chat deleted event', async () => {
    let onChatDeletedHandler: ChatListEventHandlers['onChatDeleted'];
    vi.mocked(mockChatApi.onChatListEvents).mockImplementation((handlers) => {
      onChatDeletedHandler = handlers.onChatDeleted;
      return () => {};
    });

    await store.init();
    expect(store.chats.length).toBe(1);

    onChatDeletedHandler!({ chatId: 1, chatVersion: 1 });

    expect(store.chats.length).toBe(0);
  });

  it('should cleanup on clear', async () => {
    const cleanup = vi.fn();
    vi.mocked(mockChatApi.onChatListEvents).mockReturnValue(cleanup);

    await store.init();
    store.clear();

    expect(cleanup).toHaveBeenCalled();
    expect(store.chats).toEqual([]);
  });
});