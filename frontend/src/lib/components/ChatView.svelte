<script lang="ts">
  import { currentChat, allMessages, chatLoading, chatError } from '../stores/chat.js';
  import Message from './Message.svelte';
  import MessageInput from './MessageInput.svelte';

  function dismissError() {
    chatError.set(null);
  }

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

  /**
   * Handle message sent event.
   */
  function handleMessageSent(): void {
    // Scroll to bottom after sending
    setTimeout(() => scrollToBottom(), 0);
  }

  // Auto-scroll when messages change (only if at bottom)
  $effect(() => {
    if ($allMessages && isAtBottom) {
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
    if ($allMessages && !$chatLoading) {
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
      <div class="error-content">
        <span class="text-error">{$chatError}</span>
        <button class="btn-icon" onclick={dismissError} title="Dismiss">×</button>
      </div>
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
      {#if $allMessages.length === 0}
        <div class="messages-empty">
          <span class="text-muted">No messages yet</span>
        </div>
      {:else}
        <div class="messages-list">
          {#each $allMessages as message (message.id)}
            <Message {message} isQueue={message.isQueue} />
          {/each}
        </div>
      {/if}
    </div>

    <MessageInput onMessageSent={handleMessageSent} />
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
  .chat-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .chat-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: var(--spacing-lg);
  }

  .error-content {
    padding: var(--spacing-md);
    background: var(--color-error-bg);
    border: 1px solid var(--color-error);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .btn-icon {
    background: none;
    border: none;
    color: var(--color-error);
    font-size: var(--font-size-lg);
    cursor: pointer;
    padding: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
  }

  .btn-icon:hover {
    background: rgba(0, 0, 0, 0.1);
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

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
