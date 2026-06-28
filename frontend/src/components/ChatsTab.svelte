<script>
  import { onMount } from 'svelte';
  import { currentChatId } from '../lib/chatStores.js';
  import { loadChats } from '../lib/chatWs.js';
  import ChatList from './ChatList.svelte';
  import ChatView from './ChatView.svelte';

  onMount(() => {
    loadChats();
  });
</script>

<div class="chats-tab">
  <ChatList />
  {#if $currentChatId}
    <ChatView />
  {:else}
    <div class="placeholder">
      <p class="placeholder-text">Select or create a chat</p>
    </div>
  {/if}
</div>

<style>
  .chats-tab {
    display: flex;
    height: calc(100vh - 100px);
  }

  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .placeholder-text {
    color: var(--color-text-muted);
    font-size: 16px;
  }
</style>
