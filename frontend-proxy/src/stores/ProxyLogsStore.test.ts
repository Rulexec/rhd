import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ProxyLogsStore } from './ProxyLogsStore.js';
import { ApiError } from '../lib/api/ProxyLogsApi.js';
import type { ProxyLogsApi } from '../lib/api/ProxyLogsApi.js';
import type {
  ChatDetail,
  ChatSummary,
  ChatsResponse,
  RequestDetail,
  RequestSummary
} from '../lib/api/schemas.js';

/**
 * Unit tests for ProxyLogsStore.
 *
 * Async methods are generator prototype methods that makeAutoObservable
 * auto-wraps into flows, so calling e.g. `store.loadChats()` returns a
 * CancellablePromise directly and can be awaited.
 */

function chatSummary(id: number, overrides: Partial<ChatSummary> = {}): ChatSummary {
  return {
    id,
    title: `Chat ${id}`,
    model: 'test-model',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    requestCount: 2,
    ...overrides
  };
}

function requestSummary(chatId: number, id: number): RequestSummary {
  return {
    id,
    chatId,
    ts: '2026-01-01T00:00:00Z',
    method: 'POST',
    path: '/v1/chat/completions',
    model: 'test-model',
    stream: false,
    status: 200,
    durationMs: 42,
    error: null,
    hasAssembled: true
  };
}

function chatDetail(chatId: number, requestIds: number[], overrides: Partial<ChatDetail> = {}): ChatDetail {
  return {
    chat: chatSummary(chatId),
    requests: requestIds.map((id) => requestSummary(chatId, id)),
    conversation: [
      {
        kind: 'message',
        seq: 0,
        role: 'user',
        source: 'history',
        content: 'hello',
        toolCalls: null,
        toolCallId: null,
        name: null,
        requestId: requestIds[0] ?? 0
      },
      {
        kind: 'message',
        seq: 1,
        role: 'assistant',
        source: 'response',
        content: 'hi',
        toolCalls: null,
        toolCallId: null,
        name: null,
        requestId: requestIds.at(-1) ?? 0
      }
    ],
    ...overrides
  };
}

function requestDetail(id: number, overrides: Partial<RequestDetail> = {}): RequestDetail {
  return {
    summary: requestSummary(1, id),
    requestBody: '{"model":"test-model","messages":[]}',
    responseBody: '{"id":"resp-1"}',
    responseAssembled: '{"role":"assistant","content":"hi"}',
    ...overrides
  };
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe('ProxyLogsStore', () => {
  let store: ProxyLogsStore;
  let mockApi: ProxyLogsApi;

  beforeEach(() => {
    mockApi = {
      listChats: vi.fn().mockResolvedValue({ chats: [chatSummary(1), chatSummary(2)] }),
      getChatDetail: vi.fn().mockImplementation((chatId: number) =>
        Promise.resolve(chatDetail(chatId, [10, 11]))
      ),
      getRequestDetail: vi.fn().mockImplementation((requestId: number) =>
        Promise.resolve(requestDetail(requestId))
      )
    } as unknown as ProxyLogsApi;

    store = new ProxyLogsStore({ api: mockApi });
  });

  it('should initialize with empty state', () => {
    expect(store.chats).toEqual([]);
    expect(store.hasChats).toBe(false);
    expect(store.chatsLoading).toBe(false);
    expect(store.chatsError).toBe(null);
    expect(store.selectedChatId).toBe(null);
    expect(store.chatDetail).toBe(null);
    expect(store.detailLoading).toBe(false);
    expect(store.detailError).toBe(null);
    expect(store.selectedRequestId).toBe(null);
    expect(store.requestDetail).toBe(null);
    expect(store.refreshing).toBe(false);
  });

  describe('loadChats', () => {
    it('should populate the list and toggle chatsLoading', async () => {
      const pending = deferred<ChatsResponse>();
      vi.mocked(mockApi.listChats).mockReturnValueOnce(pending.promise);

      const flow = store.loadChats();
      expect(store.chatsLoading).toBe(true);

      pending.resolve({ chats: [chatSummary(1), chatSummary(2)] });
      await flow;

      expect(store.chats).toHaveLength(2);
      expect(store.hasChats).toBe(true);
      expect(store.chatsLoading).toBe(false);
      expect(store.chatsError).toBe(null);
    });

    it('should set chatsError and keep the list on failure', async () => {
      vi.mocked(mockApi.listChats).mockRejectedValueOnce(new Error('boom'));

      await store.loadChats();

      expect(store.chatsError).toBe('boom');
      expect(store.chatsLoading).toBe(false);
      expect(store.chats).toEqual([]);
    });
  });

  describe('openChat', () => {
    it('should select the chat and load its detail', async () => {
      await store.loadChats();
      await store.openChat(1);

      expect(store.selectedChatId).toBe(1);
      expect(store.chatDetail?.chat.id).toBe(1);
      expect(store.detailLoading).toBe(false);
      expect(store.detailError).toBe(null);
      expect(store.selectedChat?.id).toBe(1);
    });

    it('should drop an open request when opening another chat', async () => {
      await store.loadChats();
      await store.openChat(1);
      await store.openRequest(10);
      expect(store.selectedRequestId).toBe(10);

      await store.openChat(2);

      expect(store.selectedChatId).toBe(2);
      expect(store.selectedRequestId).toBe(null);
      expect(store.requestDetail).toBe(null);
    });

    it('should set detailError and keep chatDetail null on failure', async () => {
      vi.mocked(mockApi.getChatDetail).mockRejectedValueOnce(new Error('detail boom'));

      await store.openChat(1);

      expect(store.detailError).toBe('detail boom');
      expect(store.chatDetail).toBe(null);
      expect(store.detailLoading).toBe(false);
    });
  });

  describe('openRequest', () => {
    it('should select the request and load its detail', async () => {
      await store.openChat(1);
      await store.openRequest(10);

      expect(store.selectedRequestId).toBe(10);
      expect(store.requestDetail?.summary.id).toBe(10);
    });

    it('should set detailError on failure', async () => {
      await store.openChat(1);
      vi.mocked(mockApi.getRequestDetail).mockRejectedValueOnce(new Error('req boom'));

      await store.openRequest(10);

      expect(store.detailError).toBe('req boom');
      expect(store.requestDetail).toBe(null);
    });
  });

  describe('closeRequest', () => {
    it('should clear the request selection but keep the chat selection', async () => {
      await store.loadChats();
      await store.openChat(1);
      await store.openRequest(10);

      store.closeRequest();

      expect(store.selectedRequestId).toBe(null);
      expect(store.requestDetail).toBe(null);
      expect(store.selectedChatId).toBe(1);
      expect(store.chatDetail?.chat.id).toBe(1);
    });
  });

  describe('refresh', () => {
    it('should refetch only the chat list when nothing is selected', async () => {
      const pending = deferred<ChatsResponse>();
      vi.mocked(mockApi.listChats).mockReturnValueOnce(pending.promise);

      const flow = store.refresh();
      expect(store.refreshing).toBe(true);

      pending.resolve({ chats: [chatSummary(1)] });
      await flow;

      expect(store.refreshing).toBe(false);
      expect(mockApi.listChats).toHaveBeenCalledTimes(1);
      expect(mockApi.getChatDetail).not.toHaveBeenCalled();
      expect(mockApi.getRequestDetail).not.toHaveBeenCalled();
    });

    it('should preserve chat and request selection with fresh data', async () => {
      await store.loadChats();
      await store.openChat(1);
      await store.openRequest(10);

      vi.mocked(mockApi.listChats).mockResolvedValueOnce({
        chats: [chatSummary(2), chatSummary(1, { updatedAt: '2026-01-02T00:00:00Z' })]
      });
      vi.mocked(mockApi.getChatDetail).mockResolvedValueOnce(chatDetail(1, [10, 11, 12]));
      vi.mocked(mockApi.getRequestDetail).mockResolvedValueOnce(
        requestDetail(10, { responseBody: '{"id":"resp-refreshed"}' })
      );

      await store.refresh();

      expect(mockApi.listChats).toHaveBeenCalledTimes(2);
      expect(mockApi.getChatDetail).toHaveBeenCalledTimes(2);
      expect(mockApi.getRequestDetail).toHaveBeenCalledTimes(2);
      expect(store.selectedChatId).toBe(1);
      expect(store.selectedRequestId).toBe(10);
      expect(store.chatDetail?.requests.map((r) => r.id)).toEqual([10, 11, 12]);
      expect(store.requestDetail?.responseBody).toBe('{"id":"resp-refreshed"}');
      expect(store.chatsError).toBe(null);
    });

    it('should clear the selection when the selected chat disappeared', async () => {
      await store.loadChats();
      await store.openChat(1);

      vi.mocked(mockApi.listChats).mockResolvedValueOnce({ chats: [chatSummary(2)] });

      await store.refresh();

      expect(store.selectedChatId).toBe(null);
      expect(store.chatDetail).toBe(null);
      expect(store.selectedRequestId).toBe(null);
      expect(store.requestDetail).toBe(null);
      expect(mockApi.getChatDetail).toHaveBeenCalledTimes(1); // not refetched
      expect(store.chatsError).toBe(null);
    });

    it('should close the request when it disappeared from the refreshed detail', async () => {
      await store.loadChats();
      await store.openChat(1);
      await store.openRequest(10);

      vi.mocked(mockApi.getChatDetail).mockResolvedValueOnce(chatDetail(1, [11]));

      await store.refresh();

      expect(store.selectedChatId).toBe(1);
      expect(store.selectedRequestId).toBe(null);
      expect(store.requestDetail).toBe(null);
      expect(mockApi.getRequestDetail).toHaveBeenCalledTimes(1); // not refetched
    });

    it('should clear the selection without chatsError on a 404', async () => {
      await store.loadChats();
      await store.openChat(1);

      vi.mocked(mockApi.getChatDetail).mockRejectedValueOnce(new ApiError('chat not found', 404));

      await store.refresh();

      expect(store.selectedChatId).toBe(null);
      expect(store.chatDetail).toBe(null);
      expect(store.chatsError).toBe(null);
    });

    it('should set chatsError and keep the selection on a non-404 failure', async () => {
      await store.loadChats();
      await store.openChat(1);

      vi.mocked(mockApi.getChatDetail).mockRejectedValueOnce(new ApiError('internal error', 500));

      await store.refresh();

      expect(store.chatsError).toBe('internal error');
      expect(store.selectedChatId).toBe(1);
      expect(store.refreshing).toBe(false);
    });
  });

  describe('clearSelection', () => {
    it('should reset everything selection-related', async () => {
      await store.loadChats();
      await store.openChat(1);
      await store.openRequest(10);

      store.clearSelection();

      expect(store.selectedChatId).toBe(null);
      expect(store.chatDetail).toBe(null);
      expect(store.detailError).toBe(null);
      expect(store.selectedRequestId).toBe(null);
      expect(store.requestDetail).toBe(null);
      expect(store.chats).toHaveLength(2); // list is not selection state
    });
  });
});
