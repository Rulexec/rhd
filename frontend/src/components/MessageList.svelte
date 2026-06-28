<script>
  import { onMount, afterUpdate } from 'svelte';
  import { messages, isStreaming, streamingContent } from '../lib/chatStores.js';
  import Message from './Message.svelte';
  import StreamingMessage from './StreamingMessage.svelte';

  let listElement;

  afterUpdate(() => {
    if (listElement) {
      listElement.scrollTop = listElement.scrollHeight;
    }
  });
</script>

<div class="message-list" bind:this={listElement}>
  {#each $messages as message (message.id)}
    <Message {message} />
  {/each}
  {#if $isStreaming}
    <StreamingMessage content={$streamingContent} />
  {/if}
</div>

<style>
  .message-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-l);
  }
</style>
