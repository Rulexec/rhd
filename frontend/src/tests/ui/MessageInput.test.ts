/**
 * Test cases covered:
 * - tests/cases/chat-send-message.md (UI rendering steps)
 * - tests/cases/chat-abort.md (UI rendering steps)
 * - tests/cases/chat-pause-resume.md (UI rendering steps)
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

  // Covers chat-send-message.md step 2 (UI rendering)
  // Step 2. User clicks Send button (renders Send button when not streaming)
  it('renders send button when not streaming', () => {
    render(MessageInput);
    expect(screen.getByText('Send')).toBeTruthy();
  });

  // Covers chat-abort.md step 1 (UI rendering)
  // Step 1. User clicks Abort button (renders Abort button when streaming)
  it('renders abort button when streaming', () => {
    isStreaming.set(true);
    render(MessageInput);
    expect(screen.getByText('Abort')).toBeTruthy();
  });

  // Covers chat-pause-resume.md pause step 1 (UI rendering)
  // Pause Step 1. User clicks Pause button (renders Pause button when streaming and not paused)
  it('renders pause button when streaming and not paused', () => {
    isStreaming.set(true);
    isPaused.set(false);
    render(MessageInput);
    expect(screen.getByText('Pause')).toBeTruthy();
  });

  // Covers chat-pause-resume.md resume step 1 (UI rendering)
  // Resume Step 1. User clicks Resume button (renders Resume button when paused)
  it('renders resume button when paused', () => {
    isStreaming.set(true);
    isPaused.set(true);
    render(MessageInput);
    expect(screen.getByText('Resume')).toBeTruthy();
  });

  // Covers chat-send-message.md preconditions (UI state validation)
  // Preconditions: Chat exists and is selected (disables Send when no chat selected)
  it('disables send button when no chat selected', () => {
    currentChatId.set(null);
    render(MessageInput);
    const sendButton = screen.getByText('Send');
    expect(sendButton.hasAttribute('disabled')).toBe(true);
  });

  // Covers chat-send-message.md preconditions (UI state validation)
  // Preconditions: Model is selected (disables Send when no model selected)
  it('disables send button when no model selected', () => {
    selectedModel.set(null);
    render(MessageInput);
    const sendButton = screen.getByText('Send');
    expect(sendButton.hasAttribute('disabled')).toBe(true);
  });

  // Covers chat-send-message.md steps 12-13 area (error state UI)
  // Step 12-13. System sets isStreaming to false, receives error (UI shows error message and Retry button)
  it('shows error message when streamError is set', () => {
    streamError.set('Test error message');
    render(MessageInput);
    expect(screen.getByText('Test error message')).toBeTruthy();
    expect(screen.getByText('Retry')).toBeTruthy();
  });

  // Covers chat-send-message.md preconditions (UI rendering)
  // Preconditions: Model is selected (renders model selector with available models)
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
