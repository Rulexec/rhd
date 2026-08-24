<script lang="ts">
  import { currentChatId } from '../stores/chat.js';
  import { connectionStore } from '../stores/connection.js';
  import { addQueueMessage } from '../api/chatApi.js';

  interface Props {
    onMessageSent?: () => void;
  }

  let { onMessageSent }: Props = $props();

  let inputValue: string = $state('');
  let textareaEl: HTMLTextAreaElement | null = $state(null);
  let isSending: boolean = $state(false);
  let errorMessage: string | null = $state(null);

  let isConnected: boolean = $derived($connectionStore.status === 'connected');
  let canSend: boolean = $derived(
    inputValue.trim().length > 0 && $currentChatId != null && isConnected && !isSending
  );

  /**
   * Auto-resize textarea based on content.
   */
  function autoResize(): void {
    if (!textareaEl) return;

    // Reset height to auto to get correct scrollHeight
    textareaEl.style.height = 'auto';

    // Set height to scrollHeight (max 200px)
    const newHeight = Math.min(textareaEl.scrollHeight, 200);
    textareaEl.style.height = `${newHeight}px`;
  }

  /**
   * Handle input event.
   */
  function handleInput(): void {
    autoResize();
    errorMessage = null;
  }

  /**
   * Handle keydown event.
   */
  function handleKeydown(event: KeyboardEvent): void {
    // Enter without Shift sends message
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleSend();
    }
    // Shift+Enter adds new line (default behavior)
  }

  /**
   * Send message to queue.
   */
  async function handleSend(): Promise<void> {
    if (!canSend || $currentChatId == null) return;

    const content = inputValue.trim();
    if (!content) return;

    if (!isConnected) {
      errorMessage = 'Not connected to server';
      return;
    }

    isSending = true;
    errorMessage = null;

    try {
      await addQueueMessage($currentChatId, 'user', content);
      inputValue = '';

      // Reset textarea height
      if (textareaEl) {
        textareaEl.style.height = 'auto';
      }

      onMessageSent?.();
    } catch (error) {
      // Provide more specific error messages
      const errorText = error instanceof Error ? error.message : String(error);
      if (errorText.includes('WebSocket not connected')) {
        errorMessage = 'Lost connection to server. Please reconnect.';
      } else if (errorText.includes('timeout')) {
        errorMessage = 'Request timed out. Please try again.';
      } else {
        errorMessage = errorText || 'Failed to send message';
      }
    } finally {
      isSending = false;
    }
  }
</script>

<div class="message-input">
  {#if errorMessage}
    <div class="input-error">
      <span class="text-error">{errorMessage}</span>
    </div>
  {/if}

  <div class="input-container">
    <textarea
      bind:this={textareaEl}
      bind:value={inputValue}
      oninput={handleInput}
      onkeydown={handleKeydown}
      placeholder="Type a message... (Enter to send, Shift+Enter for new line)"
      disabled={!isConnected || $currentChatId == null}
      rows="1"
      class="input-textarea"
    ></textarea>

    <button
      class="send-button"
      class:active={canSend}
      disabled={!canSend}
      onclick={handleSend}
      title="Send message"
    >
      {isSending ? '⏳' : '📤'}
    </button>
  </div>

  {#if $currentChatId == null}
    <div class="input-hint">
      <span class="text-muted">Select a chat to send messages</span>
    </div>
  {:else if !isConnected}
    <div class="input-hint">
      <span class="text-error">Not connected to server</span>
    </div>
  {/if}
</div>

<style>
  .message-input {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .input-error {
    padding: var(--spacing-xs) var(--spacing-sm);
    margin-bottom: var(--spacing-sm);
    background: var(--color-error-bg);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
  }

  .input-container {
    display: flex;
    gap: var(--spacing-sm);
    align-items: flex-end;
  }

  .input-textarea {
    flex: 1;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg);
    color: var(--color-text);
    font-size: var(--font-size-md);
    font-family: inherit;
    line-height: 1.5;
    resize: none;
    overflow: hidden;
    min-height: 40px;
    max-height: 200px;
  }

  .input-textarea:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .input-textarea:disabled {
    background: var(--color-bg-tertiary);
    cursor: not-allowed;
  }

  .send-button {
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    border-radius: var(--radius-md);
    background: var(--color-bg-tertiary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-lg);
    cursor: not-allowed;
    transition: all var(--transition-fast);
  }

  .send-button.active {
    background: var(--color-primary);
    color: white;
    cursor: pointer;
  }

  .send-button.active:hover {
    background: var(--color-primary-hover);
  }

  .send-button:disabled {
    opacity: 0.5;
  }

  .input-hint {
    margin-top: var(--spacing-xs);
    font-size: var(--font-size-xs);
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
