<script lang="ts">
  import { afterUpdate } from 'svelte';
  import { messages, isStreaming, streamingContent, streamingMessageId } from '../lib/chatStores';
  import Message from './Message.svelte';
  import StreamingMessage from './StreamingMessage.svelte';

  let listElement: HTMLDivElement;

  afterUpdate(() => {
    if (listElement) {
      listElement.scrollTop = listElement.scrollHeight;
    }
  });

  function shouldShowModelIndicator(index: number): boolean {
    if (index === 0) return true;
    const currentModel = $messages[index].model;
    const previousModel = $messages[index - 1].model;
    return currentModel !== previousModel;
  }
</script>

<div class="message-list" bind:this={listElement}>
  {#each $messages as message, index (message.id)}
    {#if shouldShowModelIndicator(index)}
      <div class="model-indicator">
        <span class="model-indicator-text">Model: {message.model || 'Unknown'}</span>
      </div>
    {/if}
    <Message {message} />
  {/each}
  {#if $isStreaming && !$streamingMessageId}
    <StreamingMessage content={$streamingContent} />
  {/if}
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
</style>
