<script lang="ts">
  import { currentChatId } from '../lib/chatStores';
  import { wsConnected } from '../lib/stores';
  import { loadChats, selectChat } from '../lib/chatWs';
  import { parseHash } from '../lib/router';
  import ChatList from './ChatList.svelte';
  import ChatView from './ChatView.svelte';

  let chatsLoaded = $state(false);

  $effect(() => {
    if ($wsConnected && !chatsLoaded) {
      chatsLoaded = true;
      loadChats().then(() => {
        const parsed = parseHash();
        if (parsed.chatId && parsed.tab === 'chats') {
          selectChat(parsed.chatId);
        }
      });
    }
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
