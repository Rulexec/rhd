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
    {
      kind: 'message',
      seq: 0,
      role: 'system',
      source: 'history',
      content: 'You are helpful.',
      toolCalls: null,
      toolCallId: null,
      name: null,
      requestId: 11
    },
    {
      kind: 'message',
      seq: 1,
      role: 'user',
      source: 'history',
      content: 'Ping',
      toolCalls: null,
      toolCallId: null,
      name: null,
      requestId: 11
    },
    {
      kind: 'message',
      seq: 2,
      role: 'assistant',
      source: 'response',
      content: 'Pong **done**',
      toolCalls: null,
      toolCallId: null,
      name: null,
      requestId: 12
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
  openRequestAnchor?: number | 'tail' | null;
}) {
  const openRequest = vi.fn().mockResolvedValue(undefined);
  const closeRequest = vi.fn();
  const mockStore = {
    chatDetail: options.chatDetail ?? null,
    detailLoading: options.detailLoading ?? false,
    detailError: options.detailError ?? null,
    requestDetail: options.requestDetail ?? null,
    selectedRequestId: options.selectedRequestId ?? null,
    openRequestAnchor: options.openRequestAnchor ?? null,
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

  it('should render header, conversation turns, collapsed timeline, and index panel', () => {
    const { store } = createMockStore({ chatDetail });

    const { getByText, getByTestId, container } = render(ChatDetailViewHarness, {
      props: { store }
    });

    expect(getByText('Debug chat')).toBeTruthy();
    expect(getByText(/z-ai\/glm-5\.3/)).toBeTruthy();
    expect(getByTestId('turn-system')).toBeTruthy();
    expect(getByTestId('turn-user')).toBeTruthy();
    expect(getByTestId('turn-assistant').textContent).toContain('Pong');
    // Timeline header visible but rows collapsed by default (the index panel
    // also uses role=option, so assert on the timeline's own listbox).
    expect(getByTestId('requests-toggle').textContent).toContain('Requests (2)');
    expect(container.querySelector('ul[aria-label="Requests"]')).toBeNull();
    // Index panel lists user messages and tool-less assistant responses.
    const index = getByTestId('conversation-index');
    expect(index.textContent).toContain('Ping');
    expect(index.textContent).toContain('Pong');
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

  it('should call openRequest with the row id and a null anchor when a timeline row is clicked', async () => {
    const { store, openRequest } = createMockStore({ chatDetail });

    const { getByTestId, getByText } = render(ChatDetailViewHarness, { props: { store } });

    await fireEvent.click(getByTestId('requests-toggle'));
    await fireEvent.click(getByText('req-model-a'));

    // Always two arguments, so assertions are uniform across placements.
    expect(openRequest).toHaveBeenCalledWith(11, null);
  });

  it('should call openRequest with the turn requestId and seq when a raw button is clicked', async () => {
    const { store, openRequest } = createMockStore({ chatDetail });

    const { getAllByTestId } = render(ChatDetailViewHarness, { props: { store } });

    const buttons = getAllByTestId('turn-request-button');
    expect(buttons).toHaveLength(3);
    await fireEvent.click(buttons[2]!);

    expect(openRequest).toHaveBeenCalledWith(12, 2);
  });

  it('should close instead of re-open when the same request is already open at the same anchor', async () => {
    const { store, openRequest, closeRequest } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11,
      openRequestAnchor: 1
    });

    const { getAllByTestId } = render(ChatDetailViewHarness, { props: { store } });

    // The user turn (seq 1, requestId 11) is the current anchor.
    await fireEvent.click(getAllByTestId('turn-request-button')[1]!);

    expect(closeRequest).toHaveBeenCalledTimes(1);
    expect(openRequest).not.toHaveBeenCalled();
  });

  it('should re-anchor (not close) when a sibling turn of the same request is clicked', async () => {
    const { store, openRequest, closeRequest } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11,
      openRequestAnchor: 1
    });

    const { getAllByTestId } = render(ChatDetailViewHarness, { props: { store } });

    // The system turn carries the same requestId 11 but a different seq.
    await fireEvent.click(getAllByTestId('turn-request-button')[0]!);

    expect(closeRequest).not.toHaveBeenCalled();
    expect(openRequest).toHaveBeenCalledTimes(1);
    expect(openRequest).toHaveBeenCalledWith(11, 0);
  });

  it('should render an anchored detail inline in the conversation, not in the bottom section', () => {
    const { store } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11,
      openRequestAnchor: 1
    });

    const { getByTestId, container } = render(ChatDetailViewHarness, { props: { store } });

    const detailEl = getByTestId('request-detail');
    const conversation = container.querySelector('.conversation')!;
    // Exactly one drill-down exists, and it lives inside the conversation.
    expect(container.querySelectorAll('[data-testid="request-detail"]')).toHaveLength(1);
    expect(conversation.contains(detailEl)).toBe(true);
    // Between the anchored turn and the following turns.
    const children = Array.from(conversation.children);
    expect(children.indexOf(container.querySelector('[data-turn-seq="0"]')!)).toBeLessThan(
      children.indexOf(detailEl)
    );
    expect(children.indexOf(container.querySelector('[data-turn-seq="1"]')!)).toBe(
      children.indexOf(detailEl) - 1
    );
    expect(children.indexOf(container.querySelector('[data-turn-seq="2"]')!)).toBe(
      children.indexOf(detailEl) + 1
    );
  });

  it('should render an un-anchored detail in the bottom section', () => {
    const { store } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11,
      openRequestAnchor: null
    });

    const { getByTestId, container } = render(ChatDetailViewHarness, { props: { store } });

    const detailEl = getByTestId('request-detail');
    expect(container.querySelector('.conversation')?.contains(detailEl)).toBe(false);
    // It is the last section of the scroll column, below the timeline.
    const sections = Array.from(container.querySelectorAll('.chat-detail-section'));
    expect(sections.at(-1)?.contains(detailEl)).toBe(true);
  });

  it('should fall back to the bottom section when the anchored turn no longer exists', () => {
    const { store } = createMockStore({
      chatDetail,
      requestDetail,
      selectedRequestId: 11,
      openRequestAnchor: 99
    });

    const { getByTestId, container } = render(ChatDetailViewHarness, { props: { store } });

    const detailEl = getByTestId('request-detail');
    expect(container.querySelector('.conversation')?.contains(detailEl)).toBe(false);
    const sections = Array.from(container.querySelectorAll('.chat-detail-section'));
    expect(sections.at(-1)?.contains(detailEl)).toBe(true);
  });
});
