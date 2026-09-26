import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChatsListHarness from './ChatsListHarness.svelte';
import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';
import type { ChatSummary } from '../api/schemas.js';

const chat = (id: number, overrides: Partial<ChatSummary> = {}): ChatSummary => ({
  id,
  title: `Chat ${id}`,
  model: 'z-ai/glm-5.3',
  createdAt: '2026-09-27T10:00:00Z',
  updatedAt: '2026-09-27T10:05:00Z',
  requestCount: 3,
  ...overrides
});

/**
 * Build a mocked ProxyLogsStore for ChatsList component tests.
 * `mobxObservable`'s autorun runs once against plain getters, which is
 * enough for static render assertions.
 */
function createMockStore(options: {
  chats: ChatSummary[];
  loading?: boolean;
  error?: string | null;
}) {
  const mockStore = {
    _chats: options.chats,
    chatsLoading: options.loading ?? false,
    chatsError: options.error ?? null,
    get chats() {
      return [...this._chats];
    },
    get hasChats() {
      return this._chats.length > 0;
    }
  };
  const store = mockStore as unknown as ProxyLogsStore;
  return { store };
}

afterEach(() => {
  cleanup();
});

describe('ChatsList', () => {
  it('should render empty state when no chats and not loading', () => {
    const { store } = createMockStore({ chats: [] });

    const { getByText } = render(ChatsListHarness, { props: { store } });

    expect(getByText('No logged chats yet — send traffic through rhd_ai_proxy and press Refresh')).toBeTruthy();
  });

  it('should render loading state and hide the list while loading', () => {
    const { store } = createMockStore({ chats: [chat(1)], loading: true });

    const { getByText, queryByText } = render(ChatsListHarness, { props: { store } });

    expect(getByText('Loading chats...')).toBeTruthy();
    expect(queryByText('Chat 1')).toBeNull();
  });

  it('should render error text when chatsError is set', () => {
    const { store } = createMockStore({ chats: [chat(1)], error: 'failed to load chats' });

    const { getByText, queryByText } = render(ChatsListHarness, { props: { store } });

    expect(getByText('failed to load chats')).toBeTruthy();
    expect(queryByText('Chat 1')).toBeNull();
  });

  it('should render chats with title, model, and request count', () => {
    const { store } = createMockStore({ chats: [chat(1), chat(2)] });

    const { getByText, getAllByText } = render(ChatsListHarness, { props: { store } });

    expect(getByText('Chat 1')).toBeTruthy();
    expect(getByText('Chat 2')).toBeTruthy();
    expect(getAllByText('z-ai/glm-5.3')).toHaveLength(2);
    expect(getAllByText('3 req')).toHaveLength(2);
  });

  it('should highlight the selected chat row', () => {
    const { store } = createMockStore({ chats: [chat(1), chat(2)] });

    const { container } = render(ChatsListHarness, {
      props: { store, selectedChatId: 2 }
    });

    const selected = container.querySelector('[aria-selected="true"]');
    expect(selected).toBeTruthy();
    expect(selected?.textContent).toContain('Chat 2');
    expect(container.querySelectorAll('[aria-selected="true"]')).toHaveLength(1);
  });

  it('should dispatch chatSelect with chatId on row click', async () => {
    const { store } = createMockStore({ chats: [chat(1)] });
    const onChatSelect = vi.fn();

    const { getByText } = render(ChatsListHarness, {
      props: { store, onChatSelect }
    });

    await fireEvent.click(getByText('Chat 1'));

    expect(onChatSelect).toHaveBeenCalledWith({ chatId: 1 });
  });

  it('should render a chat without model without crashing', () => {
    const { store } = createMockStore({ chats: [chat(1, { model: null })] });

    const { getByText, queryByText } = render(ChatsListHarness, { props: { store } });

    expect(getByText('Chat 1')).toBeTruthy();
    expect(queryByText('z-ai/glm-5.3')).toBeNull();
  });
});
