<script>
  import { currentChat } from '../lib/chatStores.js';
  import MessageList from './MessageList.svelte';
  import MessageInput from './MessageInput.svelte';
</script>

<div class="chat-view">
  <div class="header">
    <h2>{$currentChat?.title || 'Chat'}</h2>
  </div>
  <MessageList />
  <MessageInput />
</div>

<style>
  .chat-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .header {
    padding: var(--spacing-m) var(--spacing-l);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg);
  }

  .header h2 {
    font-size: 18px;
    font-weight: 600;
    margin: 0;
  }
</style>
