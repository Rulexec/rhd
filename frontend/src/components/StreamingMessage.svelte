<script lang="ts">
  import { tick } from 'svelte';

  export let content: string;
  export let thinkingContent: string = '';
  
  let thinkingExpanded = false;
  let thinkingEl: HTMLPreElement | null = null;
  let thinkingAutoScroll = true;
  const SCROLL_THRESHOLD = 30;
  
  $: hasThinkingContent = thinkingContent && thinkingContent.length > 0;

  function toggleThinking() {
    thinkingExpanded = !thinkingExpanded;
    if (thinkingExpanded) {
      thinkingAutoScroll = true;
      tick().then(() => {
        if (thinkingEl) thinkingEl.scrollTop = thinkingEl.scrollHeight;
      });
    }
  }

  function handleThinkingScroll() {
    if (!thinkingEl) return;
    const { scrollTop, scrollHeight, clientHeight } = thinkingEl;
    thinkingAutoScroll = scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
  }

  function autoScrollThinking() {
    if (thinkingExpanded && thinkingEl && thinkingAutoScroll) {
      thinkingEl.scrollTop = thinkingEl.scrollHeight;
    }
  }

  $: thinkingContent, autoScrollThinking();
</script>

<div class="message assistant streaming">
  {#if hasThinkingContent}
    <button class="thinking-header" on:click={toggleThinking}>
      <span class="toggle-icon">{thinkingExpanded ? '▼' : '▶'}</span>
      <span class="thinking-label">Thinking</span>
    </button>
    {#if thinkingExpanded}
      <pre class="thinking-content" bind:this={thinkingEl} on:scroll={handleThinkingScroll}>{thinkingContent}</pre>
    {:else}
      <div class="thinking-preview">
        <pre class="thinking-content">{thinkingContent}</pre>
      </div>
    {/if}
  {/if}
  {#if content}
    <div class="content">{content}</div>
  {:else}
    <div class="loading">
      <span class="dot"></span>
      <span class="dot"></span>
      <span class="dot"></span>
    </div>
  {/if}
</div>

<style>
  .message {
    margin-bottom: var(--spacing-m);
    padding: var(--spacing-m);
    border-radius: 8px;
    margin-right: 40px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
  }

  .content {
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.6;
  }

  .loading {
    display: flex;
    gap: 4px;
    padding: var(--spacing-s) 0;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-text-muted);
    animation: bounce 1.4s infinite ease-in-out both;
  }

  .dot:nth-child(1) {
    animation-delay: -0.32s;
  }

  .dot:nth-child(2) {
    animation-delay: -0.16s;
  }

  @keyframes bounce {
    0%, 80%, 100% {
      transform: scale(0);
    }
    40% {
      transform: scale(1);
    }
  }

  .thinking-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs, 4px);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    font-size: 13px;
    color: var(--color-text-secondary, #666);
    width: 100%;
    text-align: left;
    margin-bottom: var(--spacing-xs, 4px);
  }

  .thinking-header:hover {
    color: var(--color-text, #212529);
  }

  .toggle-icon {
    font-size: 10px;
    width: 12px;
  }

  .thinking-label {
    font-weight: 500;
    font-family: monospace;
    font-style: italic;
  }

  .thinking-preview {
    max-height: calc(2 * 18px + 16px);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    margin-bottom: var(--spacing-s, 8px);
  }

  .thinking-preview .thinking-content {
    margin-bottom: 0;
    max-height: none;
    overflow: visible;
  }

  .thinking-content {
    background: var(--color-bg-secondary, #f8f9fa);
    border: 1px solid var(--color-border, #e0e0e0);
    border-radius: 4px;
    padding: var(--spacing-s, 8px);
    margin-bottom: var(--spacing-s, 8px);
    font-family: monospace;
    font-size: 12px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    max-height: 300px;
    overflow-y: auto;
    color: var(--color-text-secondary, #666);
    font-style: italic;
  }
</style>
