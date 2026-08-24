# Phase 6: Polish & Error Handling

## Overview

Add connection status UI, comprehensive error handling, and styling refinements. This phase polishes the application for production use and ensures robust error handling throughout.

**Scope:**
- Connection status indicator with reconnect button
- Inline error messages near relevant components
- Loading states for all async operations
- Styling refinements (hover states, focus states, responsive design)
- Error handling for all operations

**Out of Scope:**
- New features (all core features implemented in previous phases)
- Dark/light theme toggle (CSS variables prepared, but not implementing toggle)
- Advanced animations or transitions

## Dependencies

- **Requires**: All previous phases (1-5)
- **Blocks**: None (final phase)

## Files to Create/Modify

### 1. `frontend/src/lib/components/ConnectionStatus.svelte`

**Purpose:** Connection status indicator with reconnect button.

```svelte
<script>
  import { connectionStore } from '../stores/connection.js';
  import { websocket } from '../api/websocket.js';

  let isReconnecting = false;

  $: status = $connectionStore.status;
  $: error = $connectionStore.error;
  $: isConnected = status === 'connected';
  $: isDisconnected = status === 'disconnected';
  $: isConnecting = status === 'connecting';

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
        on:click={handleReconnect}
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
```

### 2. `frontend/src/lib/api/websocket.js`

**Modify:** Improve error handling and add better reconnection support.

```javascript
// Modify the connect() method to add better error handling

/**
 * Connect to the WebSocket server.
 */
connect() {
  if (this.ws && (this.ws.readyState === WebSocket.CONNECTING || this.ws.readyState === WebSocket.OPEN)) {
    console.warn('WebSocket already connected or connecting');
    return;
  }

  this.errorMessage = null;
  connectionStore.set({ status: 'connecting', error: null });

  try {
    this.ws = new WebSocket(WS_URL);

    this.ws.onopen = () => {
      console.log('WebSocket connected');
      this.isConnected = true;
      this.errorMessage = null;
      connectionStore.set({ status: 'connected', error: null });
    };

    this.ws.onclose = (event) => {
      console.log('WebSocket disconnected:', event.code, event.reason);
      this.isConnected = false;
      this.ws = null;
      this.rejectAllPending('WebSocket connection closed');
      
      // Provide more specific error message based on close code
      let errorMessage = 'Connection closed';
      if (event.code === 1006) {
        errorMessage = 'Connection lost unexpectedly';
      } else if (event.reason) {
        errorMessage = event.reason;
      }
      
      connectionStore.set({ status: 'disconnected', error: errorMessage });
    };

    this.ws.onerror = (event) => {
      console.error('WebSocket error:', event);
      this.errorMessage = 'Connection error';
      connectionStore.set({ status: 'disconnected', error: 'Connection error occurred' });
    };

    this.ws.onmessage = (event) => {
      this.handleMessage(event.data);
    };
  } catch (error) {
    console.error('Failed to create WebSocket:', error);
    this.errorMessage = 'Failed to create WebSocket connection';
    connectionStore.set({ status: 'disconnected', error: this.errorMessage });
  }
}

// Add a method to check connection health
/**
 * Check if WebSocket is connected and ready.
 * @returns {boolean}
 */
isReady() {
  return this.isConnected && this.ws && this.ws.readyState === WebSocket.OPEN;
}
```

### 3. `frontend/src/lib/stores/connection.js`

**Modify:** Add error message tracking and helper methods.

```javascript
/**
 * Re-export connection store from websocket module.
 * This provides a convenient import path for components.
 */
export { connectionStore } from '../api/websocket.js';

/**
 * Helper function to check if connected.
 * @param {import('svelte/store').Readable<{status: string, error: string | null}>} store
 * @returns {boolean}
 */
export function isConnected(store) {
  let connected = false;
  store.subscribe(({ status }) => {
    connected = status === 'connected';
  })();
  return connected;
}

/**
 * Helper function to get connection error.
 * @param {import('svelte/store').Readable<{status: string, error: string | null}>} store
 * @returns {string | null}
 */
export function getConnectionError(store) {
  let error = null;
  store.subscribe(({ error: err }) => {
    error = err;
  })();
  return error;
}
```

### 4. `frontend/src/lib/components/ChatList.svelte`

**Modify:** Add error handling for chat operations.

```svelte
<script>
  // ... existing imports ...
  import { connectionStore } from '../stores/connection.js';

  // ... existing code ...

  $: isConnected = $connectionStore.status === 'connected';

  // Modify handleCreateChat to show error
  async function handleCreateChat() {
    if (!isConnected) {
      return;
    }

    isCreating = true;
    try {
      const chat = await createNewChat();
      if (chat) {
        dispatch('chatSelect', { chatId: chat.id });
      }
    } catch (error) {
      // Error is already set in chatsError store
      console.error('Failed to create chat:', error);
    } finally {
      isCreating = false;
    }
  }

  // Modify handleDeleteAllConfirm to show error
  async function handleDeleteAllConfirm() {
    showDeleteAllModal = false;
    isDeleting = true;
    try {
      const success = await deleteAllChats();
      if (!success) {
        // Error is already set in chatsError store
        console.error('Failed to delete all chats');
      }
    } catch (error) {
      console.error('Failed to delete all chats:', error);
    } finally {
      isDeleting = false;
    }
  }
</script>

<!-- Add error display in the template -->
<div class="chat-list">
  <div class="chat-list-header">
    <button
      class="{commonStyles['btn']} {commonStyles['btn-primary']} {commonStyles['btn-sm']}"
      disabled={isCreating || !isConnected}
      on:click={handleCreateChat}
    >
      {isCreating ? 'Creating...' : '+ New Chat'}
    </button>
  </div>

  {#if $chatsError}
    <div class="chat-list-error">
      <span class="text-error">{$chatsError}</span>
      <button class="btn-icon" on:click={() => chatsError.set(null)} title="Dismiss">×</button>
    </div>
  {/if}

  <!-- ... rest of existing template ... -->
</div>

<style>
  /* Add error styling */
  .chat-list-error {
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--color-error-bg);
    border-bottom: 1px solid var(--color-error);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--font-size-sm);
  }
</style>
```

### 5. `frontend/src/lib/components/ChatView.svelte`

**Modify:** Add error handling for message operations.

```svelte
<script>
  // ... existing imports ...
  import { chatError } from '../stores/chat.js';

  // ... existing code ...
</script>

<div class="chat-view">
  {#if $chatLoading}
    <div class="chat-loading">
      <span class="text-muted">Loading chat...</span>
    </div>
  {:else if $chatError}
    <div class="chat-error">
      <div class="error-content">
        <span class="text-error">{$chatError}</span>
        <button class="btn-icon" on:click={() => chatError.set(null)} title="Dismiss">×</button>
      </div>
    </div>
  {:else if !$currentChat}
    <!-- ... existing empty state ... -->
  {:else}
    <!-- ... existing chat view ... -->
  {/if}
</div>

<style>
  .chat-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: var(--spacing-lg);
  }

  .error-content {
    padding: var(--spacing-md);
    background: var(--color-error-bg);
    border: 1px solid var(--color-error);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }
</style>
```

### 6. `frontend/src/lib/components/MessageInput.svelte`

**Modify:** Improve error handling and add connection check.

```svelte
<script>
  // ... existing imports ...
  import { connectionStore } from '../stores/connection.js';

  // ... existing code ...

  $: isConnected = $connectionStore.status === 'connected';
  $: canSend = inputValue.trim().length > 0 && $currentChatId && isConnected && !isSending;

  // Modify handleSend to provide better error messages
  async function handleSend() {
    if (!canSend) return;

    const content = inputValue.trim();
    if (!content) return;

    if (!isConnected) {
      errorMessage = 'Not connected to server';
      return;
    }

    isSending = true;
    errorMessage = null;

    try {
      await addQueueMessage($currentChatId, 'user', content);
      inputValue = '';
      
      // Reset textarea height
      if (textareaEl) {
        textareaEl.style.height = 'auto';
      }
      
      dispatch('messageSent');
    } catch (error) {
      // Provide more specific error messages
      if (error.message.includes('WebSocket not connected')) {
        errorMessage = 'Lost connection to server. Please reconnect.';
      } else if (error.message.includes('timeout')) {
        errorMessage = 'Request timed out. Please try again.';
      } else {
        errorMessage = error.message || 'Failed to send message';
      }
    } finally {
      isSending = false;
    }
  }
</script>
```

### 7. `frontend/src/App.svelte`

**Modify:** Integrate ConnectionStatus component.

```svelte
<script>
  // ... existing imports ...
  import ConnectionStatus from './lib/components/ConnectionStatus.svelte';

  // ... existing code ...
</script>

<div class="app">
  <ConnectionStatus />
  
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>

  <!-- ... rest of existing template ... -->
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
    /* Add padding when connection status is shown */
  }

  /* Adjust header when connection status banner is visible */
  :global(.connection-status) ~ .app-header {
    margin-top: 40px; /* Approximate height of status banner */
  }
</style>
```

### 8. `frontend/src/global.css`

**Modify:** Add error state styles, refine spacing and colors, add responsive design.

```css
/* Add these additional styles to global.css */

/* ============================================================================
 * Error States
 * ============================================================================ */

.error-banner {
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-error-bg);
  border: 1px solid var(--color-error);
  border-radius: var(--radius-md);
  color: var(--color-error);
  font-size: var(--font-size-sm);
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
}

.error-banner-dismissible {
  justify-content: space-between;
}

/* ============================================================================
 * Loading States
 * ============================================================================ */

.loading-spinner {
  display: inline-block;
  width: 20px;
  height: 20px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

/* ============================================================================
 * Focus States (Accessibility)
 * ============================================================================ */

*:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

button:focus-visible,
input:focus-visible,
textarea:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

/* ============================================================================
 * Hover States
 * ============================================================================ */

.clickable {
  cursor: pointer;
  transition: all var(--transition-fast);
}

.clickable:hover {
  opacity: 0.8;
}

/* ============================================================================
 * Responsive Design
 * ============================================================================ */

@media (max-width: 768px) {
  :root {
    --spacing-md: 12px;
    --spacing-lg: 16px;
  }

  .chats-layout {
    flex-direction: column;
  }

  .chats-sidebar {
    width: 100%;
    max-height: 40vh;
    border-right: none;
    border-bottom: 1px solid var(--color-border);
  }

  .chats-content {
    flex: 1;
    min-height: 0;
  }
}

@media (max-width: 480px) {
  :root {
    --font-size-md: 14px;
    --font-size-lg: 16px;
  }

  .app-header h1 {
    font-size: var(--font-size-md);
  }
}

/* ============================================================================
 * Scrollbar Styling
 * ============================================================================ */

::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: var(--color-bg-secondary);
}

::-webkit-scrollbar-thumb {
  background: var(--color-border);
  border-radius: var(--radius-sm);
}

::-webkit-scrollbar-thumb:hover {
  background: var(--color-text-muted);
}

/* ============================================================================
 * Selection Styling
 * ============================================================================ */

::selection {
  background: var(--color-primary);
  color: white;
}
```

## Tests

### Manual Testing

1. **Connection Status:**
   - Start app with server running - verify no status banner shown
   - Stop server - verify "Disconnected" banner appears with error message
   - Click "Reconnect" button - verify reconnection attempt
   - Start server - verify "Connected" banner appears briefly then disappears
   - Verify banner is fixed at top of page

2. **Error Handling:**
   - Create chat while disconnected - verify error shown in chat list
   - Send message while disconnected - verify error shown in message input
   - Load chat with network error - verify error shown in chat view
   - Verify errors are dismissible (click × button)
   - Verify errors clear when operation succeeds

3. **Loading States:**
   - Load chat list - verify loading indicator shown
   - Load chat messages - verify loading indicator shown
   - Load plugins - verify loading indicator shown
   - Send message - verify send button shows loading state

4. **Responsive Design:**
   - Resize browser to mobile width (< 768px)
   - Verify chat list moves to top
   - Verify chat view moves to bottom
   - Verify layout is usable on mobile

5. **Accessibility:**
   - Tab through all interactive elements
   - Verify focus indicators are visible
   - Verify keyboard navigation works
   - Verify ARIA labels are present

6. **Styling Refinements:**
   - Hover over buttons - verify hover states
   - Click buttons - verify active states
   - Scroll lists - verify custom scrollbar styling
   - Select text - verify selection styling

7. **Edge Cases:**
   - Rapidly switch tabs - verify no errors
   - Send multiple messages quickly - verify all sent correctly
   - Disconnect during message send - verify error handled gracefully
   - Delete all chats while loading - verify no errors

## Implementation Notes

1. **Connection Status Banner**: Fixed position at top of page. Only shown when not connected. Provides clear visual feedback and manual reconnect option.

2. **Error Dismissal**: Errors can be dismissed by clicking the × button. Errors also auto-clear when the operation succeeds or when the component unmounts.

3. **Loading Spinners**: CSS-only loading spinners using border animation. Lightweight and performant.

4. **Focus States**: All interactive elements have visible focus indicators for keyboard navigation. Uses `:focus-visible` to only show focus when using keyboard (not mouse).

5. **Responsive Breakpoints**:
   - 768px: Chat list moves to top, chat view to bottom
   - 480px: Font sizes reduced for mobile

6. **Custom Scrollbars**: WebKit scrollbar styling for consistent appearance across browsers. Falls back to default scrollbars in Firefox.

7. **Error Messages**: Provide specific, actionable error messages:
   - "Lost connection to server. Please reconnect."
   - "Request timed out. Please try again."
   - "Not connected to server"

8. **Connection Health Check**: Added `isReady()` method to WebSocket client to check if connection is truly ready (not just connected, but OPEN state).

## Dependencies

- **Requires**: All previous phases (1-5)
- **Blocks**: None (final phase)

## Success Criteria

- [ ] Connection status indicator shows current state
- [ ] Reconnect button works when disconnected
- [ ] Error messages appear inline near relevant components
- [ ] Errors are dismissible
- [ ] Loading states shown during async operations
- [ ] UI is responsive on mobile devices
- [ ] All operations have proper error handling
- [ ] Styling is consistent and polished
- [ ] Application handles edge cases gracefully (network errors, empty states, disconnections)
- [ ] Focus states visible for keyboard navigation
- [ ] Hover states provide visual feedback
- [ ] Custom scrollbars styled consistently
- [ ] Error messages are specific and actionable
- [ ] Connection status banner doesn't overlap content
