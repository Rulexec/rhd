<script lang="ts">
  import { onMount } from 'svelte';
  import { isStreaming, streamError, currentChatId, availableModels, selectedModel, isPaused, isAborted } from '@/lib/chatStores';
  import { chatProjects, mcpStatuses } from '@/lib/projectStores';
  import { dispatch } from '@/lib/actions';
  import { TEST_IDS } from '@/stories/testIds';

  let input = '';
  let textareaElement: HTMLTextAreaElement;
  let isPausePending = false;

  $: if ($availableModels.length > 0 && $selectedModel === null) {
    selectedModel.set($availableModels[0]);
  }

  $: if ($isPaused) {
    isPausePending = false;
  }

  $: hasMcpError = $chatProjects.length > 0 && $mcpStatuses.some(
    (s) => $chatProjects.some((p) => p.name === s.projectName) && s.status === 'failed'
  );

  $: canSend = $selectedModel && !$isStreaming;
  $: canQueue = $selectedModel && ($isPaused || $isAborted);
  $: showPauseButton = $isStreaming && !$isPaused;
  $: showAbortButton = $isStreaming && !$isPaused;
  $: showResumeButton = $isPaused || $isAborted;

  onMount(() => {
    dispatch({ type: 'loadAvailableModels' });
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleSend();
    }
  }

  function handleSend() {
    if (canQueue) {
      dispatch({ type: 'queueMessage', payload: { content: input.trim(), model: $selectedModel! } });
      input = '';
    } else if (canSend) {
      dispatch({ type: 'sendMessage', payload: { content: input.trim(), model: $selectedModel! } });
      input = '';
    }
    if (textareaElement) {
      textareaElement.style.height = 'auto';
    }
  }

  function abort() {
    dispatch({ type: 'abortChat' });
  }

  function pause() {
    isPausePending = true;
    dispatch({ type: 'pauseChat' });
  }

  function resume() {
    dispatch({ type: 'resumeChat' });
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
      placeholder={$isPaused || $isAborted ? 'Type a message to queue...' : 'Type a message...'}
      disabled={!canSend && !canQueue}
      class="input-textarea"
      rows="1"
      data-testid={TEST_IDS.MESSAGE_INPUT}
    ></textarea>
    {#if showPauseButton}
      <button on:click={pause} class="pause-btn" data-testid={TEST_IDS.PAUSE_BUTTON} disabled={isPausePending}>Pause</button>
    {/if}
    
    {#if showAbortButton}
      <button on:click={abort} class="abort-btn">Abort</button>
    {/if}
    
    {#if showResumeButton}
      <button on:click={resume} class="resume-btn" data-testid={TEST_IDS.RESUME_BUTTON}>Resume</button>
    {/if}
    
    {#if canSend || canQueue}
      <button on:click={handleSend} disabled={!input.trim() || !$currentChatId || hasMcpError} class="send-btn" data-testid={TEST_IDS.SEND_BUTTON}>
        {canQueue ? 'Queue' : 'Send'}
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
  
  .pause-btn:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .resume-btn {
    background: #5cb85c;
    color: white;
  }
</style>
