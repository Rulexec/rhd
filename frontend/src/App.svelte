<script lang="ts">
  import { onMount } from 'svelte';
  import { websocket } from './lib/api/websocket.js';
  import { defaultChatApi } from './lib/api/ChatApi.js';
  import { AppStore } from './stores/AppStore.js';
  import { setAppStore } from './context.js';
  import { mobxObservable } from './util/mobxObservable.svelte.js';
  import TabView from './lib/components/TabView.svelte';
  import ChatList from './lib/components/ChatList.svelte';
  import ChatView from './lib/components/ChatView.svelte';
  import PluginList from './lib/components/PluginList.svelte';
  import ConnectionStatus from './lib/components/ConnectionStatus.svelte';

  type TabType = 'chats' | 'plugins';

  // Initialize the AppStore and provide it to all children via context
  const appStore = new AppStore({ chatApi: defaultChatApi });
  setAppStore(appStore);

  // Wire WebSocket lifecycle updates into the ConnectionStore
  websocket.setConnectionStore(appStore.connection);

  let activeTab: TabType = $state('chats');
  let selectedChatId: number | null = $state(null);

  // Bridge MobX connection status to Svelte reactivity.
  // mobxObservable must be invoked at component top level (it registers onDestroy).
  const connectionStatusGetter = mobxObservable(() => appStore.connection.status);
  let connectionStatus = $derived(connectionStatusGetter());

  onMount(() => {
    websocket.connect();
  });

  // Initialize chat list and plugins once connected
  $effect(() => {
    if (connectionStatus === 'connected') {
      appStore.chatsList.init();
      appStore.plugins.init();
    }
  });

  function handleTabChange(event: CustomEvent<{ tab: TabType }>) {
    activeTab = event.detail.tab;
  }

  async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
    selectedChatId = event.detail.chatId;
    await appStore.chat.selectChat(selectedChatId);
  }
</script>

<div class="app">
  <ConnectionStatus />

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
      <PluginList />
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

</style>
