<script lang="ts">
  import { tick } from 'svelte';
  import { dispatch } from '../lib/actions';
  import { streamingMessageId, selectedModel } from '../lib/chatStores';
  import type { ChatMessage } from '../lib/types/index';
  import { renderMarkdown } from '../lib/markdown';

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

<style>
  .message {
    margin-bottom: var(--spacing-m);
    padding: var(--spacing-m);
    border-radius: 8px;
    position: relative;
  }

  .message.user {
    background: var(--color-bg-active);
    margin-left: 40px;
  }

  .message.assistant {
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    margin-right: 40px;
  }

  .content {
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.6;
  }

  .streaming-dots {
    display: inline-block;
    width: 1.5em;
    text-align: left;
    animation: dots 0.6s steps(3, end) infinite;
  }

  .streaming-dots::after {
    content: '.';
    animation: dots-content 0.6s steps(3, end) infinite;
  }

  @keyframes dots-content {
    0% { content: '.'; }
    33% { content: '..'; }
    66% { content: '...'; }
    100% { content: '.'; }
  }

  .edit-mode {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-s);
  }

  .edit-textarea {
    width: 100%;
    min-height: 80px;
    padding: var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 14px;
    resize: vertical;
  }

  .edit-actions {
    display: flex;
    gap: var(--spacing-s);
    align-items: center;
  }

  .save-btn,
  .cancel-btn {
    padding: var(--spacing-xs) var(--spacing-m);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .save-btn {
    background: var(--color-primary);
    color: white;
  }

  .cancel-btn {
    background: var(--color-border);
    color: var(--color-text);
  }

  .hint {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .edit-btn {
    position: absolute;
    top: var(--spacing-xs);
    right: var(--spacing-xs);
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    font-size: 16px;
    padding: var(--spacing-xs);
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .message.user:hover .edit-btn {
    opacity: 1;
  }

  .edit-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--color-text);
  }

  .message.system {
    background: var(--color-bg-secondary, #f8f9fa);
    border: 1px solid var(--color-border, #e0e0e0);
    padding: var(--spacing-s, 8px);
  }

  .message.system.system-collapsed {
    padding: var(--spacing-xs, 4px) var(--spacing-s, 8px);
  }

  .system-header {
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
  }

  .system-header:hover {
    color: var(--color-text, #212529);
  }

  .toggle-icon {
    font-size: 10px;
    width: 12px;
  }

  .system-label {
    font-weight: 500;
    font-family: monospace;
  }

  .system-content {
    background: var(--color-bg, #fff);
    border: 1px solid var(--color-border, #e0e0e0);
    border-radius: 4px;
    padding: var(--spacing-s, 8px);
    margin-top: var(--spacing-xs, 4px);
    font-family: monospace;
    font-size: 12px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    max-height: 400px;
    overflow-y: auto;
    color: var(--color-text, #212529);
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

  .markdown-toggle-btn {
    position: absolute;
    top: var(--spacing-xs);
    right: var(--spacing-xs);
    background: none;
    border: 1px solid var(--color-border);
    color: var(--color-text-muted);
    cursor: pointer;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.2s;
    font-family: monospace;
  }

  .message.assistant:hover .markdown-toggle-btn {
    opacity: 1;
  }

  .markdown-toggle-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--color-text);
  }

  .markdown-body {
    white-space: normal;
  }

  .markdown-body :global(h1),
  .markdown-body :global(h2),
  .markdown-body :global(h3),
  .markdown-body :global(h4),
  .markdown-body :global(h5),
  .markdown-body :global(h6) {
    margin-top: 1em;
    margin-bottom: 0.5em;
    font-weight: 600;
  }

  .markdown-body :global(code) {
    background: var(--color-bg-secondary, #f6f8fa);
    padding: 0.2em 0.4em;
    border-radius: 3px;
    font-size: 0.9em;
    font-family: monospace;
  }

  .markdown-body :global(pre) {
    background: var(--color-bg-secondary, #f6f8fa);
    padding: 1em;
    border-radius: 6px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .markdown-body :global(pre code) {
    background: none;
    padding: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    padding-left: 2em;
    margin: 0.5em 0;
  }

  .markdown-body :global(blockquote) {
    border-left: 4px solid var(--color-border, #ddd);
    padding-left: 1em;
    margin: 0.5em 0;
    color: var(--color-text-secondary, #666);
  }

  .markdown-body :global(a) {
    color: var(--color-primary, #0066cc);
    text-decoration: none;
  }

  .markdown-body :global(a:hover) {
    text-decoration: underline;
  }

  .markdown-body :global(table) {
    border-collapse: collapse;
    margin: 0.5em 0;
  }

  .markdown-body :global(th),
  .markdown-body :global(td) {
    border: 1px solid var(--color-border, #ddd);
    padding: 0.5em;
  }

  .markdown-body :global(th) {
    background: var(--color-bg-secondary, #f6f8fa);
    font-weight: 600;
  }

  .thinking-content.markdown-body {
    font-style: italic;
    color: var(--color-text-secondary, #666);
  }
</style>
