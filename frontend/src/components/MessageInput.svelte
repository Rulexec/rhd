<script lang="ts">
  import { isStreaming, streamError, currentChatId } from '../lib/chatStores';
  import { sendMessage, abortChat } from '../lib/chatWs';

  let input = '';
  let model = 'gpt4';
  let textareaElement: HTMLTextAreaElement;

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }

  function send() {
    if (!input.trim() || $isStreaming || !$currentChatId) return;
    sendMessage(input.trim(), model);
    input = '';
    if (textareaElement) {
      textareaElement.style.height = 'auto';
    }
  }

  function abort() {
    abortChat();
  }

  function handleInput(event: Event) {
    if (textareaElement) {
      textareaElement.style.height = 'auto';
      textareaElement.style.height = textareaElement.scrollHeight + 'px';
    }
  }

  function retry() {
    streamError.set(null);
  }
</script>

<div class="message-input">
  {#if $streamError}
    <div class="error">
      <span>{$streamError}</span>
      <button on:click={retry} class="retry-btn">Retry</button>
    </div>
  {/if}
  <div class="input-row">
    <textarea
      bind:this={textareaElement}
      bind:value={input}
      on:keydown={handleKeydown}
      on:input={handleInput}
      placeholder="Type a message..."
      disabled={$isStreaming}
      class="input-textarea"
      rows="1"
    ></textarea>
    {#if $isStreaming}
      <button on:click={abort} class="abort-btn">Abort</button>
    {:else}
      <button on:click={send} disabled={!input.trim() || !$currentChatId} class="send-btn">
        Send
      </button>
    {/if}
  </div>
</div>

<style>
  .message-input {
    border-top: 1px solid var(--color-border);
    padding: var(--spacing-m);
    background: var(--color-bg);
  }

  .error {
    background: #fee;
    border: 1px solid #fcc;
    border-radius: 4px;
    padding: var(--spacing-s);
    margin-bottom: var(--spacing-s);
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    color: #c33;
  }

  .retry-btn {
    background: #c33;
    color: white;
    border: none;
    border-radius: 4px;
    padding: var(--spacing-xs) var(--spacing-s);
    cursor: pointer;
    font-size: 12px;
  }

  .input-row {
    display: flex;
    gap: var(--spacing-s);
    align-items: flex-end;
  }

  .input-textarea {
    flex: 1;
    padding: var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 14px;
    resize: none;
    min-height: 36px;
    max-height: 200px;
    line-height: 1.4;
  }

  .input-textarea:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .input-textarea:disabled {
    background: var(--color-bg-active);
    cursor: not-allowed;
  }

  .send-btn,
  .abort-btn {
    padding: var(--spacing-s) var(--spacing-m);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
  }

  .send-btn {
    background: var(--color-primary);
    color: white;
  }

  .send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .abort-btn {
    background: #c33;
    color: white;
  }
</style>
