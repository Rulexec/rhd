/**
 * Test cases covered:
 * - tests/cases/chat-send-message.md (UI rendering steps)
 * - tests/cases/chat-abort.md (UI rendering steps)
 * - tests/cases/chat-pause-resume.md (UI rendering steps)
 * - tests/cases/pause-abort/pause-during-ai-call.md (UI rendering steps)
 * - tests/cases/pause-abort/pause-during-tool-execution.md (UI rendering steps)
 * - tests/cases/pause-abort/abort-during-ai-call.md (UI rendering steps)
 * - tests/cases/pause-abort/abort-during-tool-execution.md (UI rendering steps)
 * - tests/cases/pause-abort/message-queue-during-pause-abort.md (UI rendering steps)
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
  isAborted,
  streamError,
  queuedMessages,
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

  // Covers pause-during-ai-call.md step 1 (UI rendering)
  // Step 1. User clicks Pause button (renders pause button when streaming and not paused)
  it('renders pause button when streaming and not paused', () => {
    isStreaming.set(true);
    isPaused.set(false);
    isAborted.set(false);
    render(MessageInput);
    expect(screen.getByText('Pause')).toBeTruthy();
    expect(screen.getByText('Abort')).toBeTruthy();
    expect(screen.queryByText('Resume')).toBeNull();
  });

  // Covers pause-during-ai-call.md step 17 (UI rendering)
  // Step 17. User clicks Resume button (renders resume button when paused)
  it('renders resume button when paused', () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(false);
    render(MessageInput);
    expect(screen.queryByText('Pause')).toBeNull();
    expect(screen.queryByText('Abort')).toBeNull();
    expect(screen.getByText('Resume')).toBeTruthy();
  });

  // Covers abort-during-ai-call.md step 20 (UI rendering)
  // Step 20. User clicks Resume button (renders resume button when aborted)
  it('renders resume button when aborted', () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(true);
    render(MessageInput);
    expect(screen.queryByText('Pause')).toBeNull();
    expect(screen.queryByText('Abort')).toBeNull();
    expect(screen.getByText('Resume')).toBeTruthy();
  });

  // Covers pause-during-ai-call.md step 10 (UI rendering)
  // Step 10. User types message in input field (allows message input when paused)
  it('allows message input when paused', async () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(false);
    render(MessageInput);
    
    const textarea = screen.getByPlaceholderText('Type a message to queue...');
    expect(textarea).toBeTruthy();
    expect(textarea.hasAttribute('disabled')).toBe(false);
    
    await fireEvent.input(textarea, { target: { value: 'Hello' } });
    expect((textarea as HTMLTextAreaElement).value).toBe('Hello');
  });

  // Covers pause-during-ai-call.md step 11 (UI rendering)
  // Step 11. User clicks Send button (shows queue button when paused)
  it('shows queue button when paused', () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(false);
    render(MessageInput);
    expect(screen.getByText('Queue')).toBeTruthy();
  });

  // Covers message-queue-during-pause-abort.md step 7 (UI rendering)
  // Step 7. System adds message to queuedMessages store with "Queued" indicator (shows queued messages indicator)
  it('shows queued messages indicator', () => {
    isPaused.set(true);
    queuedMessages.set([
      { id: 'queued-1', content: 'Hello', model: 'model1', queuedAt: new Date().toISOString(), status: 'queued' },
      { id: 'queued-2', content: 'World', model: 'model1', queuedAt: new Date().toISOString(), status: 'queued' },
    ]);
    render(MessageInput);
    expect(screen.getByText('2 messages queued')).toBeTruthy();
  });
});
