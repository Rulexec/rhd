<script lang="ts">
  import { afterUpdate } from 'svelte';
  import { messages, isStreaming, isPaused, streamingContent, streamingThinkingContent, streamingMessageId, queuedMessages } from '@/lib/chatStores';
  import Message from './Message.svelte';
  import StreamingMessage from './StreamingMessage.svelte';
  import ToolCallMessage from './ToolCallMessage.svelte';

  let listElement: HTMLDivElement;
  let isAtBottom = true;
  const SCROLL_THRESHOLD = 30;

  function handleScroll() {
    if (!listElement) return;
    const { scrollTop, scrollHeight, clientHeight } = listElement;
    isAtBottom = scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
  }

  afterUpdate(() => {
    if (listElement && isAtBottom) {
      listElement.scrollTop = listElement.scrollHeight;
    }
  });

  $: if ($isStreaming && !$streamingMessageId) {
    isAtBottom = true;
  }

  function shouldShowModelIndicator(index: number): boolean {
    if (index === 0) return true;
    const currentModel = $messages[index].model;
    const previousModel = $messages[index - 1].model;
    return currentModel !== previousModel;
  }
</script>

<div class="message-list" bind:this={listElement} on:scroll={handleScroll}>
  {#each $messages as message, index (message.id)}
    {#if shouldShowModelIndicator(index)}
      <div class="model-indicator">
        <span class="model-indicator-text">Model: {message.model || 'Unknown'}</span>
      </div>
    {/if}
    <Message {message} />
    {#if 'toolCalls' in message && message.toolCalls && message.toolCalls.length > 0}
      <div class="tool-calls-container">
        {#each message.toolCalls as toolCall (toolCall.id)}
          <ToolCallMessage {toolCall} />
        {/each}
      </div>
    {/if}
  {/each}
  {#if ($isStreaming || $isPaused) && !$streamingMessageId}
    <StreamingMessage content={$streamingContent} thinkingContent={$streamingThinkingContent} />
  {/if}
  {#each $queuedMessages as message (message.id)}
    <Message {message} />
  {/each}
</div>

<style>
  .message-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-l);
  }

  .model-indicator {
    display: flex;
    justify-content: center;
    margin: var(--spacing-m) 0;
  }

  .model-indicator-text {
    background: var(--color-bg-secondary, #f5f5f5);
    color: var(--color-text-secondary, #666);
    padding: var(--spacing-xs) var(--spacing-m);
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
    border: 1px solid var(--color-border, #ddd);
  }

  .tool-calls-container {
    margin-left: 40px;
    margin-right: 40px;
    margin-top: calc(-1 * var(--spacing-s, 8px));
    margin-bottom: var(--spacing-m);
  }
</style>
