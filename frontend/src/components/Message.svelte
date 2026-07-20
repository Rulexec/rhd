<script lang="ts">
  import { tick } from 'svelte';
  import { dispatch } from '../lib/actions';
  import { streamingMessageId, selectedModel } from '../lib/chatStores';
  import type { ChatMessage } from '../lib/types/index';
  import { renderMarkdown } from '../lib/markdown';
  import './Message.styles.css';

  export let message: ChatMessage;

  let editing = false;
  let editContent = message.content;
  let model = 'gpt4';
  let systemExpanded = false;
  let thinkingExpanded = false;
  let thinkingEl: HTMLElement | null = null;
  let thinkingAutoScroll = true;
  let markdownEnabled = true;
  const SCROLL_THRESHOLD = 30;

  $: isStreamingMessage = $streamingMessageId === message.id;
  $: isSystemMessage = message.role === 'system';
  $: isAssistantMessage = message.role === 'assistant';
  $: hasThinkingContent = message.thinkingContent && message.thinkingContent.length > 0;
  $: renderedContent = markdownEnabled && isAssistantMessage ? renderMarkdown(message.content) : null;
  $: renderedThinking = markdownEnabled && hasThinkingContent && message.thinkingContent ? renderMarkdown(message.thinkingContent) : null;

  function toggleMarkdown() {
    markdownEnabled = !markdownEnabled;
  }

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

  $: message.thinkingContent, autoScrollThinking();

  function startEdit() {
    editing = true;
    editContent = message.content;
  }

  function cancelEdit() {
    editing = false;
    editContent = message.content;
  }

  async function saveEdit() {
    if (editContent.trim() && editContent !== message.content) {
      if (typeof message.id === 'number') {
        await dispatch({ type: 'editMessage', payload: { messageId: message.id, content: editContent.trim(), model: $selectedModel || model } });
      }
    }
    editing = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      saveEdit();
    }
    if (event.key === 'Escape') {
      cancelEdit();
    }
  }
</script>

<div class="message {message.role}" class:system-collapsed={isSystemMessage && !systemExpanded}>
  {#if editing}
    <div class="edit-mode">
      <textarea
        bind:value={editContent}
        on:keydown={handleKeydown}
        class="edit-textarea"
      ></textarea>
      <div class="edit-actions">
        <button on:click={saveEdit} class="save-btn">Save</button>
        <button on:click={cancelEdit} class="cancel-btn">Cancel</button>
        <span class="hint">Ctrl+Enter to save, Esc to cancel</span>
      </div>
    </div>
  {:else if isSystemMessage}
    <button class="system-header" on:click={() => (systemExpanded = !systemExpanded)}>
      <span class="toggle-icon">{systemExpanded ? '▼' : '▶'}</span>
      <span class="system-label">System Prompt</span>
    </button>
    {#if systemExpanded}
      <pre class="system-content">{message.content}</pre>
    {/if}
  {:else}
    {#if isAssistantMessage}
      <button
        class="markdown-toggle-btn"
        on:click={toggleMarkdown}
        title={markdownEnabled ? 'Show raw text' : 'Show markdown'}
      >
        {markdownEnabled ? 'MD' : 'Raw'}
      </button>
    {/if}
    {#if hasThinkingContent}
      <button class="thinking-header" on:click={toggleThinking}>
        <span class="toggle-icon">{thinkingExpanded ? '▼' : '▶'}</span>
        <span class="thinking-label">Thinking</span>
      </button>
      {#if thinkingExpanded}
        {#if renderedThinking}
          <div class="thinking-content markdown-body" bind:this={thinkingEl} on:scroll={handleThinkingScroll} style="overflow-y: auto; max-height: 300px;">
            {@html renderedThinking}
          </div>
        {:else}
          <pre class="thinking-content" bind:this={thinkingEl} on:scroll={handleThinkingScroll}>{message.thinkingContent}</pre>
        {/if}
      {:else}
        <div class="thinking-preview">
          {#if renderedThinking}
            <div class="thinking-content markdown-body">
              {@html renderedThinking}
            </div>
          {:else}
            <pre class="thinking-content">{message.thinkingContent}</pre>
          {/if}
        </div>
      {/if}
    {/if}
    {#if renderedContent}
      <div class="content markdown-body">
        {@html renderedContent}
        {#if isStreamingMessage}
          <span class="streaming-dots"></span>
        {/if}
      </div>
    {:else}
      <div class="content">
        {message.content}
        {#if isStreamingMessage}
          <span class="streaming-dots"></span>
        {/if}
      </div>
    {/if}
    {#if message.role === 'user'}
      <button class="edit-btn" on:click={startEdit} aria-label="Edit message">
        ✎
      </button>
    {/if}
  {/if}
</div>
