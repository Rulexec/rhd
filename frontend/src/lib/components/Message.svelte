<script lang="ts">
  import { marked } from 'marked';
  import type { Message as MessageType } from '../api/schemas.js';

  interface Props {
    message: MessageType;
    isQueue?: boolean;
  }

  let { message, isQueue = false }: Props = $props();

  let showMarkdown: boolean = $state(true);
  let reasoningExpanded: boolean = $state(false);
  let reasoningContentEl: HTMLDivElement | null = $state(null);

  let hasReasoning: boolean = $derived(
    message.reasoningContent != null && message.reasoningContent.length > 0
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

  function toggleReasoning(): void {
    reasoningExpanded = !reasoningExpanded;
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

<div class="message" class:queue={isQueue}>
  <div class="message-header">
    <div class="message-meta">
      <span class="role-badge {getRoleBadgeClass(message.role)}">
        {message.role}
      </span>
      <span class="timestamp text-muted">
        {formatTimestamp(message.createdAt)}
      </span>
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
          {@html renderContent(message.reasoningContent!)}
        {:else}
          <pre>{message.reasoningContent}</pre>
        {/if}
      </div>
    </div>
  {/if}

  <div class="message-content">
    {#if showMarkdown}
      {@html renderContent(message.content)}
    {:else}
      <pre>{message.content}</pre>
    {/if}
  </div>
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
