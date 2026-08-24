<script>
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';

  // Connect to WebSocket on mount
  websocket.connect();
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>
  
  <main class="app-main">
    <p>Tab content will be rendered here (Phase 2+)</p>
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
</style>
