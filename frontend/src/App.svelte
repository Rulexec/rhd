<script lang="ts">
  import { onMount } from 'svelte';
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';
  import { initChats } from './lib/stores/chats.js';
  import { selectChat } from './lib/stores/chat.js';
  import TabView from './lib/components/TabView.svelte';
  import ChatList from './lib/components/ChatList.svelte';
  import ChatView from './lib/components/ChatView.svelte';

  type TabType = 'chats' | 'plugins';

  let activeTab: TabType = $state('chats');
  let selectedChatId: number | null = $state(null);

  onMount(() => {
    websocket.connect();

    // Wait for connection, then initialize chats
    const unsubscribe = connectionStore.subscribe(({ status }) => {
      if (status === 'connected') {
        initChats();
        unsubscribe();
      }
    });
  });

  function handleTabChange(event: CustomEvent<{ tab: TabType }>) {
    activeTab = event.detail.tab;
  }

  async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
    selectedChatId = event.detail.chatId;
    await selectChat(selectedChatId);
  }
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>

  <TabView {activeTab} on:tabChange={handleTabChange} />

  <main class="app-main">
    {#if activeTab === 'chats'}
      <div class="chats-layout">
        <aside class="chats-sidebar">
          <ChatList {selectedChatId} on:chatSelect={handleChatSelect} />
        </aside>
        <section class="chats-content">
          <ChatView />
        </section>
      </div>
    {:else if activeTab === 'plugins'}
      <div class="plugins-content">
        <p>Plugins tab will be rendered here (Phase 5)</p>
      </div>
    {/if}
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
  }

  .chats-layout {
    display: flex;
    height: 100%;
  }

  .chats-sidebar {
    width: 300px;
    flex-shrink: 0;
  }

  .chats-content {
    flex: 1;
    overflow: hidden;
  }

  .plugins-content {
    padding: var(--spacing-lg);
  }
</style>
