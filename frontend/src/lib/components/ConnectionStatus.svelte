<script lang="ts">
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import { websocket } from '../api/websocket.js';

  const appStore = getAppStore();
  const connectionStore = appStore.connection;

  let isReconnecting: boolean = $state(false);

  // Bridge MobX observables to Svelte reactivity.
  // mobxObservable returns a getter and must be invoked at component top level.
  const statusGetter = mobxObservable(() => connectionStore.status);
  const errorGetter = mobxObservable(() => connectionStore.error);

  let status = $derived(statusGetter());
  let error = $derived(errorGetter());
  let isConnected = $derived(status === 'connected');
  let isDisconnected = $derived(status === 'disconnected');
  let isConnecting = $derived(status === 'connecting');

  async function handleReconnect() {
    isReconnecting = true;
    try {
      websocket.disconnect();
      // Small delay to ensure clean disconnect
      await new Promise(resolve => setTimeout(resolve, 100));
      websocket.connect();
    } finally {
      isReconnecting = false;
    }
  }

  function getStatusText() {
    if (isConnecting) return 'Connecting...';
    if (isConnected) return 'Connected';
    if (isDisconnected) return 'Disconnected';
    return 'Unknown';
  }

  function getStatusClass() {
    if (isConnecting) return 'status-connecting';
    if (isConnected) return 'status-connected';
    if (isDisconnected) return 'status-disconnected';
    return '';
  }
</script>

{#if !isConnected}
  <div class="connection-status {getStatusClass()}" role="alert">
    <div class="status-content">
      <span class="status-icon">
        {#if isConnecting}
          ⏳
        {:else if isConnected}
          ✓
        {:else}
          ⚠️
        {/if}
      </span>
      <span class="status-text">{getStatusText()}</span>
      {#if error}
        <span class="status-error">— {error}</span>
      {/if}
    </div>
    
    {#if isDisconnected}
      <button
        class="reconnect-button"
        disabled={isReconnecting || isConnecting}
        onclick={handleReconnect}
      >
        {isReconnecting ? 'Reconnecting...' : 'Reconnect'}
      </button>
    {/if}
  </div>
{/if}

<style>
  .connection-status {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    padding: var(--spacing-sm) var(--spacing-md);
    display: flex;
    align-items: center;
    justify-content: space-between;
    z-index: 1000;
    font-size: var(--font-size-sm);
    animation: slideDown var(--transition-fast);
  }

  .status-connected {
    background: var(--color-success-bg);
    color: var(--color-success);
    border-bottom: 1px solid var(--color-success);
  }

  .status-disconnected {
    background: var(--color-error-bg);
    color: var(--color-error);
    border-bottom: 1px solid var(--color-error);
  }

  .status-connecting {
    background: var(--color-warning-bg);
    color: var(--color-warning);
    border-bottom: 1px solid var(--color-warning);
  }

  .status-content {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .status-icon {
    font-size: var(--font-size-md);
  }

  .status-text {
    font-weight: 600;
  }

  .status-error {
    font-weight: normal;
    opacity: 0.9;
  }

  .reconnect-button {
    padding: var(--spacing-xs) var(--spacing-md);
    border: 1px solid currentColor;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .reconnect-button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.2);
  }

  .reconnect-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  @keyframes slideDown {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
</style>
