<script lang="ts">
  // Placeholder shell; real layout + store wiring land in Phase 4.
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Proxy Logs</h1>
  </header>

  <main class="app-main">
    <p class="placeholder">Viewer not wired up yet — see Phase 4.</p>
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
    padding: var(--spacing-lg);
  }

  .placeholder {
    color: var(--color-text-muted);
  }
</style>
