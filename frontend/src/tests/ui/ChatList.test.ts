/**
 * Test cases covered:
 * - tests/cases/chat-create.md (UI part)
 * - tests/cases/chat-delete.md (UI part)
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ChatList from '../../components/ChatList.svelte';
import { chats, currentChatId } from '../../lib/chatStores';
import { _testClearOverrides } from '../../lib/actions';

vi.mock('../../lib/actions', async () => {
  const actual = await vi.importActual('../../lib/actions');
  return {
    ...actual,
    dispatch: vi.fn(),
  };
});

describe('ChatList UI', () => {
  beforeEach(() => {
    _testClearOverrides();
    vi.clearAllMocks();
    chats.set([]);
    currentChatId.set(null);
  });

  it('renders new chat button', () => {
    render(ChatList);
    expect(screen.getByText('+ New Chat')).toBeTruthy();
  });

  it('renders chat list items', () => {
    chats.set([
      { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
      { id: 2, title: 'Chat 2', createdAt: '', updatedAt: '', activeModel: null },
    ]);
    render(ChatList);
    expect(screen.getByText('Chat 1')).toBeTruthy();
    expect(screen.getByText('Chat 2')).toBeTruthy();
  });

  it('highlights selected chat', () => {
    chats.set([
      { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
      { id: 2, title: 'Chat 2', createdAt: '', updatedAt: '', activeModel: null },
    ]);
    currentChatId.set(1);
    render(ChatList);
    const chat1Element = screen.getByText('Chat 1').closest('.chat-item');
    expect(chat1Element?.classList.contains('selected')).toBe(true);
  });

  it('renders delete button for each chat', () => {
    chats.set([
      { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
    ]);
    render(ChatList);
    const deleteButtons = screen.getAllByLabelText('Delete chat');
    expect(deleteButtons.length).toBe(1);
  });
});
