<script lang="ts">
  import { onMount } from 'svelte';
  import { isStreaming, streamError, currentChatId, availableModels, selectedModel, isPaused } from '../lib/chatStores';
  import { chatProjects, mcpStatuses } from '../lib/projectStores';
  import { sendMessage, abortChat, pauseChat, resumeChat, loadAvailableModels } from '../lib/chatWs';

  let input = '';
  let textareaElement: HTMLTextAreaElement;

  $: if ($availableModels.length > 0 && $selectedModel === null) {
    selectedModel.set($availableModels[0]);
  }

  $: hasMcpError = $chatProjects.length > 0 && $mcpStatuses.some(
    (s) => $chatProjects.some((p) => p.name === s.projectName) && s.status === 'failed'
  );

  onMount(() => {
    loadAvailableModels();
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }

  function send() {
    if (!input.trim() || (!$isPaused && $isStreaming) || !$currentChatId || !$selectedModel || hasMcpError) return;
    sendMessage(input.trim(), $selectedModel);
    input = '';
    if (textareaElement) {
      textareaElement.style.height = 'auto';
    }
  }

  function abort() {
    abortChat();
  }

  function pause() {
    pauseChat();
  }

  function resume() {
    resumeChat();
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
  <div class="model-selector-row">
    <label for="model-select">Model:</label>
    <select
      id="model-select"
      bind:value={$selectedModel}
      disabled={$isStreaming || $availableModels.length === 0}
      class="model-select"
    >
      {#if $availableModels.length === 0}
        <option value="">No models available</option>
      {:else}
        {#each $availableModels as model}
          <option value={model}>{model}</option>
        {/each}
      {/if}
    </select>
  </div>
  <div class="input-row">
    <textarea
      bind:this={textareaElement}
      bind:value={input}
      on:keydown={handleKeydown}
      on:input={handleInput}
      placeholder="Type a message..."
      disabled={$isStreaming && !$isPaused}
      class="input-textarea"
      rows="1"
    ></textarea>
    {#if $isPaused}
      <button on:click={resume} class="resume-btn">Resume</button>
      <button on:click={send} disabled={!input.trim() || !$currentChatId || hasMcpError} class="send-btn">
        Send
      </button>
      <button on:click={abort} class="abort-btn">Abort</button>
    {:else if $isStreaming}
      <button on:click={pause} class="pause-btn">Pause</button>
      <button on:click={abort} class="abort-btn">Abort</button>
    {:else}
      <button on:click={send} disabled={!input.trim() || !$currentChatId || hasMcpError} class="send-btn">
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

  .model-selector-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-s);
    margin-bottom: var(--spacing-s);
  }

  .model-selector-row label {
    font-size: 14px;
    color: var(--color-text-secondary, #666);
  }

  .model-select {
    padding: var(--spacing-xs) var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-size: 14px;
    background: var(--color-bg);
    cursor: pointer;
    min-width: 150px;
  }

  .model-select:disabled {
    background: var(--color-bg-active);
    cursor: not-allowed;
    opacity: 0.6;
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
  .abort-btn,
  .pause-btn,
  .resume-btn {
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

  .pause-btn {
    background: #f0ad4e;
    color: white;
  }

  .resume-btn {
    background: #5cb85c;
    color: white;
  }
</style>
