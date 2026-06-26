<script>
  import TabNav from './components/TabNav.svelte';
  import ScenariosTab from './components/ScenariosTab.svelte';
  import ChatsTab from './components/ChatsTab.svelte';
  import { initWebSocket } from './lib/ws.js';

  let activeTab = $state('scenarios');

  initWebSocket();
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
