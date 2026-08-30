<script lang="ts">
  import { marked } from 'marked';
  import type { Message as MessageType, StreamToolCallDelta } from '../api/schemas.js';

  interface StreamContent {
    reasoningContent: string;
    content: string;
    toolCalls: StreamToolCallDelta[];
    isFinished: boolean;
  }

  interface Props {
    message: MessageType;
    isQueue?: boolean;
    streamContent?: StreamContent | null;
  }

  let { message, isQueue = false, streamContent = null }: Props = $props();

  let showMarkdown: boolean = $state(true);
  let reasoningExpanded: boolean = $state(false);
  let reasoningContentEl: HTMLDivElement | null = $state(null);
  let systemMessageExpanded: boolean = $state(false);

  let isSystemRole: boolean = $derived(message.role === 'system');

  // Determine what content to display based on streaming state
  let displayContent: string = $derived(
    streamContent && !streamContent.isFinished
      ? streamContent.content
      : message.content
  );

  let displayReasoning: string = $derived(
    streamContent && !streamContent.isFinished
      ? streamContent.reasoningContent
      : (message.reasoningContent ?? '')
  );

  let isStreaming: boolean = $derived(
    message.isStreaming && streamContent !== null && !streamContent.isFinished
  );

  let hasReasoning: boolean = $derived(
    displayReasoning != null && displayReasoning.length > 0
  );

  /**
   * Render content as Markdown or plain text.
   */
  function renderContent(content: string): string {
    if (!showMarkdown) {
      return content;
    }
    try {
      return marked.parse(content, { breaks: true }) as string;
    } catch (error) {
      console.error('Markdown rendering failed:', error);
      return content;
    }
  }

  /**
   * Scroll reasoning content to bottom when collapsed.
   */
  function scrollReasoningToBottom(): void {
    if (reasoningContentEl && !reasoningExpanded) {
      reasoningContentEl.scrollTop = reasoningContentEl.scrollHeight;
    }
  }

  // Scroll to bottom when reasoning content changes
  $effect(() => {
    if (hasReasoning && !reasoningExpanded) {
      scrollReasoningToBottom();
    }
  });

  // Scroll to bottom when streaming reasoning content updates
  $effect(() => {
    if (isStreaming && hasReasoning && !reasoningExpanded) {
      scrollReasoningToBottom();
    }
  });

  function toggleReasoning(): void {
    reasoningExpanded = !reasoningExpanded;
  }

  function toggleSystemMessage(): void {
    systemMessageExpanded = !systemMessageExpanded;
  }

  function toggleMarkdown(): void {
    showMarkdown = !showMarkdown;
  }

  function formatTimestamp(dateString: string): string {
    const date = new Date(dateString);
    return date.toLocaleString();
  }

  function getRoleBadgeClass(role: string): string {
    switch (role) {
      case 'user': return 'role-user';
      case 'assistant': return 'role-assistant';
      case 'system': return 'role-system';
      default: return 'role-default';
    }
  }
</script>

<div class="message" class:queue={isQueue} class:streaming={isStreaming}>
  <div class="message-header">
    <div class="message-meta">
      <span class="role-badge {getRoleBadgeClass(message.role)}">
        {message.role}
      </span>
      <span class="timestamp text-muted">
        {formatTimestamp(message.createdAt)}
      </span>
      {#if isStreaming}
        <span class="streaming-indicator">
          <span class="dot"></span>
          <span class="dot"></span>
          <span class="dot"></span>
        </span>
      {/if}
      {#if message.tags && message.tags.length > 0}
        <div class="message-tags">
          {#each message.tags as tag}
            <span class="tag">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>
    <button class="btn-icon markdown-toggle" onclick={toggleMarkdown} title="Toggle Markdown">
      {showMarkdown ? '📄' : '📝'}
    </button>
  </div>

  {#if hasReasoning}
    <div class="reasoning-section">
      <button class="reasoning-toggle" onclick={toggleReasoning}>
        <span class="reasoning-icon">{reasoningExpanded ? '▼' : '▶'}</span>
        <span>Reasoning</span>
      </button>
      <div
        class="reasoning-content"
        class:expanded={reasoningExpanded}
        bind:this={reasoningContentEl}
      >
        {#if showMarkdown}
          {@html renderContent(displayReasoning)}
        {:else}
          <pre>{displayReasoning}</pre>
        {/if}
      </div>
    </div>
  {/if}

  {#if isSystemRole}
    <div class="system-message-section">
      <button class="system-message-toggle" onclick={toggleSystemMessage}>
        <span class="system-message-icon">{systemMessageExpanded ? '▼' : '▶'}</span>
        <span>System Message</span>
      </button>
      <div class="system-message-content" class:expanded={systemMessageExpanded}>
        <div class="message-content">
          {#if isStreaming && !displayContent && !displayReasoning}
            <span class="streaming-placeholder">Generating response...</span>
          {:else if showMarkdown}
            {@html renderContent(displayContent)}
          {:else}
            <pre>{displayContent}</pre>
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="message-content">
      {#if isStreaming && !displayContent && !displayReasoning}
        <span class="streaming-placeholder">Generating response...</span>
      {:else if showMarkdown}
        {@html renderContent(displayContent)}
      {:else}
        <pre>{displayContent}</pre>
      {/if}
    </div>
  {/if}

  <!-- Tool calls (shown during streaming) -->
  {#if streamContent && streamContent.toolCalls.length > 0}
    <div class="tool-calls">
      {#each streamContent.toolCalls as toolCall}
        <div class="tool-call">
          <span class="tool-name">{toolCall.name}</span>
          <pre class="tool-arguments">{toolCall.arguments}</pre>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .message {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    position: relative;
  }

  .message.queue {
    opacity: 0.8;
  }

  .message.streaming {
    border-left: 3px solid var(--color-primary, #3b82f6);
    padding-left: calc(var(--spacing-md) - 3px);
  }

  .streaming-indicator {
    display: inline-flex;
    gap: 3px;
    margin-left: var(--spacing-xs);
    align-items: center;
  }

  .streaming-indicator .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: var(--color-primary, #3b82f6);
    animation: pulse 1.4s infinite;
  }

  .streaming-indicator .dot:nth-child(2) {
    animation-delay: 0.2s;
  }

  .streaming-indicator .dot:nth-child(3) {
    animation-delay: 0.4s;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 0.3;
      transform: scale(0.8);
    }
    50% {
      opacity: 1;
      transform: scale(1);
    }
  }

  .streaming-placeholder {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .tool-calls {
    margin-top: var(--spacing-sm);
    padding: var(--spacing-sm);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
  }

  .tool-call {
    margin-bottom: var(--spacing-xs);
  }

  .tool-call:last-child {
    margin-bottom: 0;
  }

  .tool-name {
    font-weight: 600;
    color: var(--color-success);
    font-size: var(--font-size-sm);
  }

  .tool-arguments {
    margin-top: var(--spacing-xs);
    padding: var(--spacing-xs);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .message-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: var(--spacing-sm);
  }

  .message-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    flex-wrap: wrap;
  }

  .role-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px var(--spacing-sm);
    border-radius: var(--radius-full);
    font-size: var(--font-size-xs);
    font-weight: 600;
    text-transform: uppercase;
  }

  .role-user {
    background: var(--color-primary);
    color: white;
  }

  .role-assistant {
    background: var(--color-success);
    color: white;
  }

  .role-system {
    background: var(--color-warning);
    color: white;
  }

  .role-default {
    background: var(--color-bg-tertiary);
    color: var(--color-text-secondary);
  }

  .timestamp {
    font-size: var(--font-size-xs);
  }

  .message-tags {
    display: flex;
    gap: var(--spacing-xs);
  }

  .markdown-toggle {
    font-size: var(--font-size-sm);
    padding: var(--spacing-xs);
  }

  .reasoning-section {
    margin-bottom: var(--spacing-sm);
  }

  .reasoning-toggle {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    padding: var(--spacing-xs) 0;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
  }

  .reasoning-toggle:hover {
    color: var(--color-text);
  }

  .reasoning-icon {
    font-size: var(--font-size-xs);
  }

  .reasoning-content {
    max-height: 4.5em; /* ~3 lines */
    overflow: hidden;
    padding: var(--spacing-sm);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    line-height: 1.5;
    transition: max-height var(--transition-normal);
  }

  .reasoning-content.expanded {
    max-height: none;
  }

  .reasoning-content pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .system-message-section {
    margin-bottom: var(--spacing-sm);
  }

  .system-message-toggle {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    padding: var(--spacing-xs) 0;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
    width: 100%;
    text-align: left;
  }

  .system-message-toggle:hover {
    color: var(--color-text);
  }

  .system-message-icon {
    font-size: var(--font-size-xs);
  }

  .system-message-content {
    max-height: 0;
    overflow: hidden;
    transition: max-height var(--transition-normal);
  }

  .system-message-content.expanded {
    max-height: none;
  }

  .message-content {
    line-height: 1.6;
  }

  .message-content pre {
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .message-content :global(p) {
    margin: 0 0 var(--spacing-sm) 0;
  }

  .message-content :global(p:last-child) {
    margin-bottom: 0;
  }

  .message-content :global(code) {
    background: var(--color-bg-tertiary);
    padding: 2px var(--spacing-xs);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
  }

  .message-content :global(pre code) {
    background: transparent;
    padding: 0;
  }

  .text-muted {
    color: var(--color-text-muted);
  }
</style>
