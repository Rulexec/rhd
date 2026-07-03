<script lang="ts">
  import { onMount } from 'svelte';
  import TabNav from './components/TabNav.svelte';
  import ScenariosTab from './components/ScenariosTab.svelte';
  import ChatsTab from './components/ChatsTab.svelte';
  import { initWebSocket } from './lib/ws';
  import { parseHash, updateHash, initRouter } from './lib/router';
  import { currentChatId } from './lib/chatStores';
  import { selectChat } from './lib/chatWs';

  let activeTab = $state<'scenarios' | 'chats'>('scenarios');

  initWebSocket();

  onMount(() => {
    const parsed = parseHash();
    activeTab = parsed.tab;

    const cleanupRouter = initRouter((parsed) => {
      if (parsed.tab !== activeTab) {
        activeTab = parsed.tab;
      }
      if (parsed.chatId && parsed.chatId !== $currentChatId && parsed.tab === 'chats') {
        selectChat(parsed.chatId);
      }
    });

    return () => {
      cleanupRouter();
    };
  });

  $effect(() => {
    const parsed = parseHash();
    if (activeTab === 'chats' && $currentChatId === null && parsed.chatId !== null) {
      return;
    }
    const chatIdForHash = activeTab === 'chats' ? $currentChatId : null;
    updateHash(activeTab, chatIdForHash);
  });
</script>

<div class="app">
  <TabNav bind:activeTab />
  <main class="main">
    {#if activeTab === 'scenarios'}
      <ScenariosTab />
    {:else}
      <ChatsTab />
    {/if}
  </main>
</div>

<style>
  .app {
    max-width: 960px;
    margin: 0 auto;
    padding: 0 16px;
  }

  .main {
    padding-top: 16px;
  }
</style>
