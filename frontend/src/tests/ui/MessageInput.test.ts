/**
 * Test cases covered:
 * - tests/cases/chat-send-message.md (UI part)
 * - tests/cases/chat-abort.md (UI part)
 * - tests/cases/chat-pause-resume.md (UI part)
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import MessageInput from '../../components/MessageInput.svelte';
import {
  currentChatId,
  availableModels,
  selectedModel,
  isStreaming,
  isPaused,
  streamError,
} from '../../lib/chatStores';
import { chatProjects, mcpStatuses } from '../../lib/projectStores';
import { _testOverrideAction, _testClearOverrides } from '../../lib/actions';
import type { ChatAction } from '../../lib/actions';

vi.mock('../../lib/actions', async () => {
  const actual = await vi.importActual('../../lib/actions');
  return {
    ...actual,
    dispatch: vi.fn(),
  };
});

describe('MessageInput UI', () => {
  beforeEach(() => {
    _testClearOverrides();
    vi.clearAllMocks();
    currentChatId.set(1);
    availableModels.set(['model1', 'model2']);
    selectedModel.set('model1');
    isStreaming.set(false);
    isPaused.set(false);
    streamError.set(null);
    chatProjects.set([]);
    mcpStatuses.set([]);
  });

  it('renders send button when not streaming', () => {
    render(MessageInput);
    expect(screen.getByText('Send')).toBeTruthy();
  });

  it('renders abort button when streaming', () => {
    isStreaming.set(true);
    render(MessageInput);
    expect(screen.getByText('Abort')).toBeTruthy();
  });

  it('renders pause button when streaming and not paused', () => {
    isStreaming.set(true);
    isPaused.set(false);
    render(MessageInput);
    expect(screen.getByText('Pause')).toBeTruthy();
  });

  it('renders resume button when paused', () => {
    isStreaming.set(true);
    isPaused.set(true);
    render(MessageInput);
    expect(screen.getByText('Resume')).toBeTruthy();
  });

  it('disables send button when no chat selected', () => {
    currentChatId.set(null);
    render(MessageInput);
    const sendButton = screen.getByText('Send');
    expect(sendButton.hasAttribute('disabled')).toBe(true);
  });

  it('disables send button when no model selected', () => {
    selectedModel.set(null);
    render(MessageInput);
    const sendButton = screen.getByText('Send');
    expect(sendButton.hasAttribute('disabled')).toBe(true);
  });

  it('shows error message when streamError is set', () => {
    streamError.set('Test error message');
    render(MessageInput);
    expect(screen.getByText('Test error message')).toBeTruthy();
    expect(screen.getByText('Retry')).toBeTruthy();
  });

  it('renders model selector with available models', () => {
    render(MessageInput);
    const modelSelect = screen.getByLabelText('Model:');
    expect(modelSelect).toBeTruthy();
    const options = Array.from(modelSelect.querySelectorAll('option')).map(
      (o) => o.textContent
    );
    expect(options).toContain('model1');
    expect(options).toContain('model2');
  });
});
