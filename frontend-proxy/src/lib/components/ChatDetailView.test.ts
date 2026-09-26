import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChatDetailViewHarness from './ChatDetailViewHarness.svelte';
import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';
import type {
  ChatDetail,
  ChatSummary,
  RequestDetail,
  RequestSummary
} from '../api/schemas.js';

const chat: ChatSummary = {
  id: 1,
  title: 'Debug chat',
  model: 'z-ai/glm-5.3',
  createdAt: '2026-09-27T10:00:00Z',
  updatedAt: '2026-09-27T10:05:00Z',
  requestCount: 2
};

const request = (id: number, model: string): RequestSummary => ({
  id,
  chatId: 1,
  ts: '2026-09-27T10:00:00Z',
  method: 'POST',
  path: '/v1/chat/completions',
  model,
  stream: false,
  status: 200,
  durationMs: 500,
  error: null,
  hasAssembled: true
});

const chatDetail: ChatDetail = {
  chat,
  requests: [request(11, 'req-model-a'), request(12, 'req-model-b')],
  conversation: [
    { kind: 'message', role: 'system', content: 'You are helpful.' },
    { kind: 'message', role: 'user', content: 'Ping' },
    {
      kind: 'assistant',
      requestId: 12,
      content: JSON.stringify({ role: 'assistant', content: 'Pong **done**' }),
      error: null,
      pending: false
    }
  ]
};

const requestDetail: RequestDetail = {
  summary: request(11, 'req-model-a'),
  requestBody: '{"model":"req-model-a"}',
  responseBody: null,
  responseAssembled: null
};

/**
 * Build a mocked ProxyLogsStore for ChatDetailView component tests.
 * `mobxObservable`'s autorun runs once against plain fields, which is enough
 * for static render assertions. openRequest must return a promise — the
 * component calls `.catch()` on flowResult(store.openRequest(...)).
 */
function createMockStore(options: {
  chatDetail?: ChatDetail | null;
  detailLoading?: boolean;
  detailError?: string | null;
  requestDetail?: RequestDetail | null;
  selectedRequestId?: number | null;
}) {
  const openRequest = vi.fn().mockResolvedValue(undefined);
  const closeRequest = vi.fn();
  const mockStore = {
    chatDetail: options.chatDetail ?? null,
    detailLoading: options.detailLoading ?? false,
    detailError: options.detailError ?? null,
    requestDetail: options.requestDetail ?? null,
    selectedRequestId: options.selectedRequestId ?? null,
    openRequest,
    closeRequest
  };
  return { store: mockStore as unknown as ProxyLogsStore, openRequest, closeRequest };
}

afterEach(() => {
  cleanup();
});

describe('ChatDetailView', () => {
  it('should render the loading state while detailLoading is true', () => {
    const { store } = createMockStore({ detailLoading: true });

    const { getByText } = render(ChatDetailViewHarness, { props: { store } });

    expect(getByText('Loading chat...')).toBeTruthy();
  });

  it('should render the error message when detailError is set', () => {
    const { store } = createMockStore({ detailError: 'failed to load chat' });

    const { getByText } = render(ChatDetailViewHarness, { props: { store } });

    expect(getByText('failed to load chat')).toBeTruthy();
  });

  it('should render header, conversation turns, and timeline for a loaded chat', () => {
    const { store } = createMockStore({ chatDetail });

    const { getByText, getByTestId } = render(ChatDetailViewHarness, { props: { store } });

    expect(getByText('Debug chat')).toBeTruthy();
    expect(getByText(/z-ai\/glm-5\.3/)).toBeTruthy();
    expect(getByTestId('turn-system')).toBeTruthy();
    expect(getByTestId('turn-user')).toBeTruthy();
    expect(getByTestId('turn-assistant').textContent).toContain('Pong');
    expect(getByText('Requests (2)')).toBeTruthy();
  });

  it('should render the request drill-down and call closeRequest on close', async () => {
    const { store, closeRequest } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11
    });

    const { getByTestId } = render(ChatDetailViewHarness, { props: { store } });

    expect(getByTestId('request-detail')).toBeTruthy();

    await fireEvent.click(getByTestId('request-detail-close'));

    expect(closeRequest).toHaveBeenCalledTimes(1);
  });

  it('should call openRequest with the row id when a timeline row is clicked', async () => {
    const { store, openRequest } = createMockStore({ chatDetail });

    const { getByText } = render(ChatDetailViewHarness, { props: { store } });

    await fireEvent.click(getByText('req-model-a'));

    expect(openRequest).toHaveBeenCalledWith(11);
  });
});
