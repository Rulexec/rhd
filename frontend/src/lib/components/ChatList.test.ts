import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChatListHarness from './ChatListHarness.svelte';
import { ConnectionStore } from '../../stores/ConnectionStore.js';
import type { AppStore } from '../../stores/AppStore.js';
import type { Chat } from '../api/schemas.js';

const mockChat: Chat = {
  id: 1,
  title: 'Test Chat',
  createdAt: '2024-01-01T00:00:00Z',
  updatedAt: '2024-01-01T00:00:00Z',
  tags: [],
  version: 1
};

/**
 * Build a mocked AppStore for ChatList component tests.
 * `chatsList` exposes observable-backed getters so mobxObservable tracks them.
 */
function createMockAppStore(options: {
  chats: Chat[];
  loading?: boolean;
  error?: string | null;
  connectionConnected?: boolean;
}) {
  const connection = new ConnectionStore();
  if (options.connectionConnected !== false) {
    connection.setConnected();
  }

  const chatsList = {
    _chats: options.chats,
    loading: options.loading ?? false,
    error: options.error ?? null,
    createNewChat: vi.fn().mockResolvedValue(2),
    deleteAllChats: vi.fn().mockResolvedValue(true),
    get chats() {
      return [...this._chats];
    },
    get hasChats() {
      return this._chats.length > 0;
    }
  };

  const appStore = {
    connection,
    chatsList
  } as unknown as AppStore;

  return { appStore, chatsList };
}

afterEach(() => {
  cleanup();
});

describe('ChatList', () => {
  it('should render empty state when no chats', () => {
    const { appStore } = createMockAppStore({ chats: [] });

    const { getByText } = render(ChatListHarness, { props: { appStore } });

    expect(getByText('No chats yet')).toBeTruthy();
  });

  it('should render chat list', () => {
    const { appStore } = createMockAppStore({ chats: [mockChat] });

    const { getByText } = render(ChatListHarness, { props: { appStore } });

    expect(getByText('Test Chat')).toBeTruthy();
    expect(getByText('Delete All Chats')).toBeTruthy();
  });

  it('should call createNewChat and dispatch chatSelect on button click', async () => {
    const { appStore, chatsList } = createMockAppStore({ chats: [] });
    const onChatSelect = vi.fn();

    const { getByText } = render(ChatListHarness, {
      props: { appStore, onChatSelect }
    });

    await fireEvent.click(getByText('+ New Chat'));

    expect(chatsList.createNewChat).toHaveBeenCalled();
    expect(onChatSelect).toHaveBeenCalledWith({ chatId: 2 });
  });

  it('should call deleteAllChats on confirm', async () => {
    const { appStore, chatsList } = createMockAppStore({ chats: [mockChat] });

    const { getByText } = render(ChatListHarness, {
      props: { appStore }
    });

    // Open the delete-all modal
    await fireEvent.click(getByText('Delete All Chats'));
    // Confirm
    await fireEvent.click(getByText('Delete All'));

    expect(chatsList.deleteAllChats).toHaveBeenCalled();
  });
});