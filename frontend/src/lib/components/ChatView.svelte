<script lang="ts">
  import { currentChat, sortedMessages, chatLoading, chatError } from '../stores/chat.js';
  import Message from './Message.svelte';

  let messagesContainer: HTMLDivElement | null = $state(null);
  let isAtBottom: boolean = $state(true);

  /**
   * Check if user is at the bottom of the message list.
   */
  function checkIfAtBottom(): boolean {
    if (!messagesContainer) return false;
    const threshold = 30;
    const { scrollTop, scrollHeight, clientHeight } = messagesContainer;
    return scrollHeight - scrollTop - clientHeight < threshold;
  }

  /**
   * Scroll to bottom of message list.
   */
  function scrollToBottom(): void {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }

  /**
   * Handle scroll event.
   */
  function handleScroll(): void {
    isAtBottom = checkIfAtBottom();
  }

  // Auto-scroll when messages change (only if at bottom)
  $effect(() => {
    if ($sortedMessages && isAtBottom) {
      // Use setTimeout to ensure DOM is updated
      setTimeout(() => {
        if (isAtBottom) {
          scrollToBottom();
        }
      }, 0);
    }
  });

  // Scroll to bottom when chat loads
  $effect(() => {
    if ($sortedMessages && !$chatLoading) {
      setTimeout(() => scrollToBottom(), 0);
    }
  });
</script>

<div class="chat-view">
  {#if $chatLoading}
    <div class="chat-loading">
      <span class="text-muted">Loading chat...</span>
    </div>
  {:else if $chatError}
    <div class="chat-error">
      <span class="text-error">{$chatError}</span>
    </div>
  {:else if !$currentChat}
    <div class="chat-empty">
      <span class="text-muted">No chat selected</span>
    </div>
  {:else}
    <div class="chat-header">
      <h2>{$currentChat.title}</h2>
      {#if $currentChat.tags && $currentChat.tags.length > 0}
        <div class="chat-tags">
          {#each $currentChat.tags as tag}
            <span class="tag">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>

    <div
      class="messages-container"
      bind:this={messagesContainer}
      onscroll={handleScroll}
    >
      {#if $sortedMessages.length === 0}
        <div class="messages-empty">
          <span class="text-muted">No messages yet</span>
        </div>
      {:else}
        <div class="messages-list">
          {#each $sortedMessages as message (message.id)}
            <Message {message} />
          {/each}
        </div>
      {/if}
    </div>

    <div class="chat-input-placeholder">
      <p class="text-muted">Message input will be added in Phase 4</p>
    </div>
  {/if}
</div>

<style>
  .chat-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .chat-loading,
  .chat-error,
  .chat-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .chat-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chat-header h2 {
    margin: 0 0 var(--spacing-xs) 0;
    font-size: var(--font-size-lg);
  }

  .chat-tags {
    display: flex;
    gap: var(--spacing-xs);
  }

  .messages-container {
    flex: 1;
    overflow-y: auto;
  }

  .messages-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .messages-list {
    display: flex;
    flex-direction: column;
  }

  .chat-input-placeholder {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
