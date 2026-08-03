/**
 * Test cases covered:
 * - tests/cases/chat-edit-message.md (UI rendering steps)
 * - tests/cases/chat-streaming.md (UI rendering steps)
 * - tests/cases/pause-abort/message-queue-during-pause-abort.md (UI rendering steps)
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import Message from '@/components/Message.svelte';
import { streamingMessageId } from '@/lib/chatStores';
import type { ChatMessage } from '@/lib/types/index';
import { _testClearOverrides } from '@/lib/actions';

vi.mock('@/lib/actions', async () => {
  const actual = await vi.importActual('@/lib/actions');
  return {
    ...actual,
    dispatch: vi.fn(),
  };
});

describe('Message UI', () => {
  beforeEach(() => {
    _testClearOverrides();
    vi.clearAllMocks();
    streamingMessageId.set(null);
  });

  // Covers chat-edit-message.md step 1 (UI rendering)
  // Step 1. User clicks edit button on user message (renders user message content)
  it('renders user message content', () => {
    const message: ChatMessage = {
      id: 1,
      chatId: 1,
      role: 'user',
      content: 'Hello, AI!',
      createdAt: '',
      model: 'model1',
    };
    render(Message, { props: { message } });
    expect(screen.getByText('Hello, AI!')).toBeTruthy();
  });

  // Covers chat-streaming.md step 8 (UI rendering)
  // Step 8. System replaces optimistic message with real message (renders assistant message content)
  it('renders assistant message content', () => {
    const message: ChatMessage = {
      id: 2,
      chatId: 1,
      role: 'assistant',
      content: 'Hello, user!',
      createdAt: '',
      model: 'model1',
    };
    render(Message, { props: { message } });
    expect(screen.getByText('Hello, user!')).toBeTruthy();
  });

  // Covers chat-edit-message.md step 1 (UI rendering)
  // Step 1. User clicks edit button on user message (renders edit button for user messages)
  it('renders edit button for user messages', () => {
    const message: ChatMessage = {
      id: 1,
      chatId: 1,
      role: 'user',
      content: 'Hello, AI!',
      createdAt: '',
      model: 'model1',
    };
    render(Message, { props: { message } });
    const editButton = screen.getByLabelText('Edit message');
    expect(editButton).toBeTruthy();
  });

  // Covers chat-edit-message.md step 1 (UI rendering)
  // Step 1. User clicks edit button on user message (edit button only for user messages, not assistant)
  it('does not render edit button for assistant messages', () => {
    const message: ChatMessage = {
      id: 2,
      chatId: 1,
      role: 'assistant',
      content: 'Hello, user!',
      createdAt: '',
      model: 'model1',
    };
    render(Message, { props: { message } });
    const editButton = screen.queryByLabelText('Edit message');
    expect(editButton).toBeNull();
  });

  // Covers chat-streaming.md step 4 (UI rendering)
  // Step 4. System displays animated dots indicator while streaming (shows streaming dots)
  it('shows streaming dots for streaming message', () => {
    const message: ChatMessage = {
      id: 'temp-123',
      chatId: 1,
      role: 'assistant',
      content: 'Streaming...',
      createdAt: '',
      model: 'model1',
    };
    streamingMessageId.set('temp-123');
    render(Message, { props: { message } });
    const dots = document.querySelector('.streaming-dots');
    expect(dots).toBeTruthy();
  });

  // Covers chat-streaming.md step 8 (UI rendering)
  // Step 8. System replaces optimistic message with real message (renders thinking content when present)
  it('renders thinking content when present', () => {
    const message: ChatMessage = {
      id: 2,
      chatId: 1,
      role: 'assistant',
      content: 'Response',
      createdAt: '',
      model: 'model1',
      thinkingContent: 'Thinking process...',
    };
    render(Message, { props: { message } });
    expect(screen.getByText('Thinking')).toBeTruthy();
  });

  // Covers chat-streaming.md step 8 (UI rendering)
  // Step 8. System replaces optimistic message with real message (renders system message with collapsible header)
  it('renders system message with collapsible header', () => {
    const message: ChatMessage = {
      id: 3,
      chatId: 1,
      role: 'system',
      content: 'System prompt content',
      createdAt: '',
      model: null,
    };
    render(Message, { props: { message } });
    expect(screen.getByText('System Prompt')).toBeTruthy();
  });

  // Covers message-queue-during-pause-abort.md step 5 (UI rendering)
  // Step 5. System optimistically adds message to pendingMessages store (renders as a gray, not-yet-sent user message)
  it('renders pending messages as gray user messages', () => {
    const pendingMessage = {
      id: 'pending-1',
      content: 'Hello',
      model: 'model1',
      queuedAt: new Date().toISOString(),
      status: 'queued' as const,
    };
    const { container } = render(Message, { props: { message: pendingMessage } });
    const element = container.querySelector('[data-message-content="Hello"]');
    expect(element).toBeTruthy();
    expect(element?.getAttribute('data-pending')).toBe('true');
    expect(element?.getAttribute('data-role')).toBe('user');
    expect(element?.classList.contains('pending')).toBe(true);
  });

  it('does not mark regular messages as pending', () => {
    const regularMessage: ChatMessage = {
      id: 1,
      chatId: 1,
      role: 'user',
      content: 'Hello',
      createdAt: '',
      model: 'model1',
    };
    const { container } = render(Message, { props: { message: regularMessage } });
    const element = container.querySelector('[data-message-content="Hello"]');
    expect(element).toBeTruthy();
    expect(element?.getAttribute('data-pending')).toBeNull();
    expect(element?.classList.contains('pending')).toBe(false);
  });
});
