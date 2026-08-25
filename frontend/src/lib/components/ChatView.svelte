<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentChat, allMessages, chatLoading, chatError } from '../stores/chat.js';
  import Message from './Message.svelte';
  import MessageInput from './MessageInput.svelte';
  import { streamSubscribe, onStreamEvents } from '../api/chatApi.js';
  import type { Message as MessageType, StreamChunkData, StreamFinishedData, StreamToolCallDelta } from '../api/schemas.js';

  function dismissError() {
    chatError.set(null);
  }

  let messagesContainer: HTMLDivElement | null = $state(null);
  let isAtBottom: boolean = $state(true);

  // Track active stream subscriptions
  interface StreamSubscription {
    reasoningContent: string;
    content: string;
    toolCalls: StreamToolCallDelta[];
    isFinished: boolean;
  }

  // Key subscriptions by messageId instead of chatId to avoid conflicts
  let streamSubscriptions: Map<number, StreamSubscription> = $state(new Map());

  // Subscribe to stream events
  let unsubscribeStreamEvents: (() => void) | null = $state(null);

  onMount(() => {
    // Subscribe to stream events for the current chat
    if ($currentChat) {
      unsubscribeStreamEvents = onStreamEvents($currentChat.id, {
        onStreamChunk: (data: StreamChunkData) => {
          const chatId = data.chatId;
          // Find the streaming message for this chat to get its messageId
          const streamingMessage = $allMessages.find(m => m.chatId === chatId && m.isStreaming);
          if (!streamingMessage) return;
          
          const subscription = streamSubscriptions.get(streamingMessage.id);
          
          if (subscription) {
            if (data.type === 'reasoningDelta' && data.content) {
              subscription.reasoningContent += data.content;
            } else if (data.type === 'contentDelta' && data.content) {
              subscription.content += data.content;
            } else if (data.type === 'toolCallDelta' && data.toolCalls) {
              // Merge tool calls by ID
              for (const tc of data.toolCalls) {
                const existing = subscription.toolCalls.find(t => t.id === tc.id);
                if (existing) {
                  existing.arguments += tc.arguments;
                } else {
                  subscription.toolCalls.push({ ...tc });
                }
              }
            }
            
            // Trigger reactivity
            streamSubscriptions = new Map(streamSubscriptions);
          }
        },
        onStreamFinished: (data: StreamFinishedData) => {
          const chatId = data.chatId;
          // Find the streaming message for this chat to get its messageId
          const streamingMessage = $allMessages.find(m => m.chatId === chatId && m.isStreaming);
          if (!streamingMessage) return;
          
          const subscription = streamSubscriptions.get(streamingMessage.id);
          
          if (subscription) {
            subscription.isFinished = true;
            // Trigger reactivity
            streamSubscriptions = new Map(streamSubscriptions);
            
            // Clean up subscription after a delay (allow final render)
            setTimeout(() => {
              streamSubscriptions.delete(streamingMessage.id);
              streamSubscriptions = new Map(streamSubscriptions);
            }, 1000);
          }
        }
      });
    }
  });

  onDestroy(() => {
    unsubscribeStreamEvents?.();
  });

  /**
   * Check if a message is streaming and subscribe to its stream if needed.
   */
  async function ensureStreamSubscription(message: MessageType) {
    if (message.isStreaming && !streamSubscriptions.has(message.id)) {
      try {
        const result = await streamSubscribe(message.chatId);
        streamSubscriptions.set(message.id, {
          reasoningContent: result.reasoningContent,
          content: result.content,
          toolCalls: result.toolCalls || [],
          isFinished: result.isFinished,
        });
        // Trigger reactivity
        streamSubscriptions = new Map(streamSubscriptions);
      } catch (error) {
        console.error('Failed to subscribe to stream:', error);
      }
    }
  }

  /**
   * Get the streaming content for a message, if any.
   */
  function getStreamContent(message: MessageType): StreamSubscription | null {
    return streamSubscriptions.get(message.id) || null;
  }

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

  // Auto-scroll when streaming content updates
  $effect(() => {
    if (streamSubscriptions.size > 0 && isAtBottom) {
      setTimeout(() => {
        if (isAtBottom) {
          scrollToBottom();
        }
      }, 0);
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
            {#if message.isStreaming}
              {#await ensureStreamSubscription(message)}
                <!-- Loading state -->
              {:then}
                <!-- Subscription established -->
              {/await}
            {/if}
            <Message {message} isQueue={message.isQueue} streamContent={getStreamContent(message)} />
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
