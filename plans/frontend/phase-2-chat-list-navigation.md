# Phase 2: Chat List & Navigation

## Overview

Implement the chat list sidebar with real-time updates, chat creation, deletion, and tab navigation. This phase provides the primary navigation interface for the application.

**Scope:**
- Chat list store with real-time updates
- Chat API methods (list, create, delete)
- Tab navigation component (Chats/Plugins)
- Chat list sidebar component
- Confirmation modal component
- Common CSS module for reusable styles

**Out of Scope:**
- Chat view / message display (Phase 3)
- Queue messages / input (Phase 4)
- Plugins tab content (Phase 5)
- Connection status UI (Phase 6)

## Dependencies

- **Requires**: Phase 1 (WebSocket foundation, connection store, Zod schemas)
- **Blocks**: Phase 3 (chat view needs chat list to select from)

## Files to Create/Modify

### 1. `frontend/src/lib/api/chatApi.js`

**Purpose:** API methods for chat operations (list, create, delete, subscribe).

```javascript
import { websocket } from './websocket.js';
import {
  ListChatsResultSchema,
  CreateChatResultSchema,
  ChatCreatedDataSchema,
  ChatUpdatedDataSchema,
  ChatDeletedDataSchema
} from './schemas.js';

/**
 * Subscribe to chat list events (chatCreated, chatUpdated, chatDeleted).
 * Must be called before listChats to receive real-time updates.
 * @returns {Promise<void>}
 */
export async function subscribeChatsList() {
  await websocket.request('subscribeChatsList', {});
}

/**
 * Unsubscribe from chat list events.
 * @returns {Promise<void>}
 */
export async function unsubscribeChatsList() {
  await websocket.request('unsubscribeChatsList', {});
}

/**
 * Get list of all chats sorted by updatedAt DESC.
 * @returns {Promise<import('./schemas.js').ListChatsResult>}
 */
export async function listChats() {
  const data = await websocket.request('listChats', {});
  return ListChatsResultSchema.parse(data);
}

/**
 * Create a new chat with the given title.
 * @param {string} title - Chat title
 * @returns {Promise<import('./schemas.js').CreateChatResult>}
 */
export async function createChat(title) {
  const data = await websocket.request('createChat', { title });
  return CreateChatResultSchema.parse(data);
}

/**
 * Delete a chat and all its messages.
 * @param {number} chatId - Chat ID to delete
 * @returns {Promise<void>}
 */
export async function deleteChat(chatId) {
  await websocket.request('deleteChat', { chatId });
}

/**
 * Generate chat title from current date/time.
 * Format: YYYY-MM-DD HH:mm
 * @returns {string}
 */
export function generateChatTitle() {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const hours = String(now.getHours()).padStart(2, '0');
  const minutes = String(now.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

/**
 * Register event listeners for chat list events.
 * Returns cleanup functions.
 * @param {Object} handlers
 * @param {Function} handlers.onChatCreated
 * @param {Function} handlers.onChatUpdated
 * @param {Function} handlers.onChatDeleted
 * @returns {Function} - Cleanup function that removes all listeners
 */
export function onChatListEvents({ onChatCreated, onChatUpdated, onChatDeleted }) {
  const unsubs = [];

  if (onChatCreated) {
    unsubs.push(websocket.on('chatCreated', (data) => {
      const parsed = ChatCreatedDataSchema.parse(data);
      onChatCreated(parsed);
    }));
  }

  if (onChatUpdated) {
    unsubs.push(websocket.on('chatUpdated', (data) => {
      const parsed = ChatUpdatedDataSchema.parse(data);
      onChatUpdated(parsed);
    }));
  }

  if (onChatDeleted) {
    unsubs.push(websocket.on('chatDeleted', (data) => {
      const parsed = ChatDeletedDataSchema.parse(data);
      onChatDeleted(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
```

### 2. `frontend/src/lib/stores/chats.js`

**Purpose:** Svelte store for chat list with reactive updates.

```javascript
import { writable, derived } from 'svelte/store';
import { subscribeChatsList, listChats, createChat, deleteChat, generateChatTitle, onChatListEvents } from '../api/chatApi.js';

/**
 * Internal store for raw chat list.
 * @type {import('svelte/store').Writable<import('../api/schemas.js').Chat[]>}
 */
const _chats = writable([]);

/**
 * Loading state.
 * @type {import('svelte/store').Writable<boolean>}
 */
export const chatsLoading = writable(false);

/**
 * Error state.
 * @type {import('svelte/store').Writable<string | null>}
 */
export const chatsError = writable(null);

/**
 * Derived store: chats sorted by updatedAt DESC.
 */
export const chats = derived(_chats, ($chats) => {
  return [...$chats].sort((a, b) => {
    return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
  });
});

/**
 * Whether there are any chats.
 */
export const hasChats = derived(chats, ($chats) => $chats.length > 0);

/**
 * Initialize the chats store: subscribe to events and load initial data.
 * Should be called once on app startup.
 */
export async function initChats() {
  chatsError.set(null);

  try {
    // Subscribe to chat list events
    await subscribeChatsList();

    // Register event listeners
    onChatListEvents({
      onChatCreated: ({ chat }) => {
        _chats.update(current => {
          // Don't add if already exists
          if (current.some(c => c.id === chat.id)) {
            return current.map(c => c.id === chat.id ? chat : c);
          }
          return [...current, chat];
        });
      },
      onChatUpdated: ({ chat }) => {
        _chats.update(current =>
          current.map(c => c.id === chat.id ? chat : c)
        );
      },
      onChatDeleted: ({ chatId }) => {
        _chats.update(current =>
          current.filter(c => c.id !== chatId)
        );
      }
    });

    // Load initial chat list
    await loadChats();
  } catch (error) {
    chatsError.set(error.message);
  }
}

/**
 * Load chats from server.
 */
export async function loadChats() {
  chatsLoading.set(true);
  chatsError.set(null);

  try {
    const result = await listChats();
    _chats.set(result.chats);
  } catch (error) {
    chatsError.set(error.message);
  } finally {
    chatsLoading.set(false);
  }
}

/**
 * Create a new chat with auto-generated title.
 * @returns {Promise<import('../api/schemas.js').Chat | null>} The created chat, or null on error
 */
export async function createNewChat() {
  chatsError.set(null);

  try {
    const title = generateChatTitle();
    const result = await createChat(title);
    // Chat will be added via chatCreated event
    return result.chat;
  } catch (error) {
    chatsError.set(error.message);
    return null;
  }
}

/**
 * Delete all chats.
 * @returns {Promise<boolean>} True if successful
 */
export async function deleteAllChats() {
  chatsError.set(null);

  try {
    // Get current chat IDs
    let currentChats;
    chats.subscribe(value => { currentChats = value; })();

    if (!currentChats || currentChats.length === 0) {
      return true;
    }

    // Delete each chat
    for (const chat of currentChats) {
      await deleteChat(chat.id);
    }

    return true;
  } catch (error) {
    chatsError.set(error.message);
    return false;
  }
}

/**
 * Clear the chats store.
 */
export function clearChats() {
  _chats.set([]);
  chatsLoading.set(false);
  chatsError.set(null);
}
```

### 3. `frontend/src/lib/styles/common.css`

**Purpose:** Common CSS module for reusable styles (buttons, lists, modals).

```css
/* ============================================================================
 * Buttons
 * ============================================================================ */

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-sm) var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-bg);
  color: var(--color-text);
  font-size: var(--font-size-sm);
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-fast);
  white-space: nowrap;
}

.btn:hover {
  background: var(--color-bg-tertiary);
}

.btn:active {
  transform: scale(0.98);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn:disabled:hover {
  background: var(--color-bg);
}

.btn-primary {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: white;
}

.btn-primary:hover {
  background: var(--color-primary-hover);
  border-color: var(--color-primary-hover);
}

.btn-danger {
  background: var(--color-error);
  border-color: var(--color-error);
  color: white;
}

.btn-danger:hover {
  background: #dc2626;
  border-color: #dc2626;
}

.btn-ghost {
  background: transparent;
  border-color: transparent;
}

.btn-ghost:hover {
  background: var(--color-bg-tertiary);
}

.btn-sm {
  padding: var(--spacing-xs) var(--spacing-sm);
  font-size: var(--font-size-xs);
}

.btn-icon {
  padding: var(--spacing-sm);
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
}

.btn-icon:hover {
  background: var(--color-bg-tertiary);
}

/* ============================================================================
 * Lists
 * ============================================================================ */

.list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.list-item {
  padding: var(--spacing-sm) var(--spacing-md);
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.list-item:hover {
  background: var(--color-bg-secondary);
}

.list-item:last-child {
  border-bottom: none;
}

.list-item.active {
  background: var(--color-bg-tertiary);
  border-left: 3px solid var(--color-primary);
}

/* ============================================================================
 * Modal
 * ============================================================================ */

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  animation: fadeIn var(--transition-fast);
}

.modal {
  background: var(--color-bg);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  min-width: 400px;
  max-width: 90vw;
  max-height: 90vh;
  overflow: auto;
  animation: slideUp var(--transition-fast);
}

.modal-header {
  padding: var(--spacing-md) var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
}

.modal-header h2 {
  margin: 0;
  font-size: var(--font-size-lg);
}

.modal-body {
  padding: var(--spacing-lg);
}

.modal-footer {
  padding: var(--spacing-md) var(--spacing-lg);
  border-top: 1px solid var(--color-border);
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-sm);
}

/* ============================================================================
 * Tags / Badges
 * ============================================================================ */

.tag {
  display: inline-flex;
  align-items: center;
  padding: 2px var(--spacing-sm);
  border-radius: var(--radius-full);
  background: var(--color-bg-tertiary);
  color: var(--color-text-secondary);
  font-size: var(--font-size-xs);
  font-weight: 500;
}

.tag-primary {
  background: var(--color-primary);
  color: white;
}

/* ============================================================================
 * Status Indicators
 * ============================================================================ */

.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
}

.status-dot-active {
  background: var(--color-success);
}

.status-dot-inactive {
  background: var(--color-text-muted);
}

/* ============================================================================
 * Animations
 * ============================================================================ */

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* ============================================================================
 * Utility Classes
 * ============================================================================ */

.text-muted {
  color: var(--color-text-muted);
}

.text-secondary {
  color: var(--color-text-secondary);
}

.text-error {
  color: var(--color-error);
}

.text-success {
  color: var(--color-success);
}

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.flex {
  display: flex;
}

.flex-col {
  flex-direction: column;
}

.items-center {
  align-items: center;
}

.justify-between {
  justify-content: space-between;
}

.gap-sm {
  gap: var(--spacing-sm);
}

.gap-md {
  gap: var(--spacing-md);
}
```

### 4. `frontend/src/lib/components/ConfirmModal.svelte`

**Purpose:** Reusable confirmation modal component.

```svelte
<script>
  import { onMount, onDestroy } from 'svelte';
  import commonStyles from '../styles/common.css?module';

  /** @type {string} */
  export let title = 'Confirm';

  /** @type {string} */
  export let message = 'Are you sure?';

  /** @type {string} */
  export let confirmText = 'Confirm';

  /** @type {string} */
  export let cancelText = 'Cancel';

  /** @type {'primary' | 'danger'} */
  export let confirmVariant = 'primary';

  /** @type {Function} */
  export let onConfirm;

  /** @type {Function} */
  export let onCancel;

  function handleKeydown(event) {
    if (event.key === 'Escape') {
      onCancel();
    }
  }

  function handleOverlayClick(event) {
    if (event.target === event.currentTarget) {
      onCancel();
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    document.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class={commonStyles['modal-overlay']} on:click={handleOverlayClick} role="dialog" aria-modal="true" aria-labelledby="modal-title">
  <div class={commonStyles['modal']}>
    <div class={commonStyles['modal-header']}>
      <h2 id="modal-title">{title}</h2>
    </div>
    <div class={commonStyles['modal-body']}>
      <p>{message}</p>
    </div>
    <div class={commonStyles['modal-footer']}>
      <button class={commonStyles['btn']} on:click={onCancel}>
        {cancelText}
      </button>
      <button
        class="{commonStyles['btn']} {confirmVariant === 'danger' ? commonStyles['btn-danger'] : commonStyles['btn-primary']}"
        on:click={onConfirm}
      >
        {confirmText}
      </button>
    </div>
  </div>
</div>
```

### 5. `frontend/src/lib/components/TabView.svelte`

**Purpose:** Tab navigation component (Chats/Plugins).

```svelte
<script>
  import { createEventDispatcher } from 'svelte';

  /** @type {'chats' | 'plugins'} */
  export let activeTab = 'chats';

  const dispatch = createEventDispatcher();

  function selectTab(tab) {
    activeTab = tab;
    dispatch('tabChange', { tab });
  }
</script>

<div class="tab-view">
  <nav class="tab-nav" role="tablist">
    <button
      class="tab-button"
      class:active={activeTab === 'chats'}
      role="tab"
      aria-selected={activeTab === 'chats'}
      on:click={() => selectTab('chats')}
    >
      Chats
    </button>
    <button
      class="tab-button"
      class:active={activeTab === 'plugins'}
      role="tab"
      aria-selected={activeTab === 'plugins'}
      on:click={() => selectTab('plugins')}
    >
      Plugins
    </button>
  </nav>
</div>

<style>
  .tab-view {
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .tab-nav {
    display: flex;
    padding: 0 var(--spacing-md);
  }

  .tab-button {
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .tab-button:hover {
    color: var(--color-text);
    background: var(--color-bg-tertiary);
  }

  .tab-button.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
  }
</style>
```

### 6. `frontend/src/lib/components/ChatList.svelte`

**Purpose:** Chat list sidebar with create/delete buttons.

```svelte
<script>
  import { chats, hasChats, chatsLoading, chatsError, createNewChat, deleteAllChats } from '../stores/chats.js';
  import ConfirmModal from './ConfirmModal.svelte';
  import commonStyles from '../styles/common.css?module';

  import { createEventDispatcher } from 'svelte';

  /** @type {number | null} */
  export let selectedChatId = null;

  const dispatch = createEventDispatcher();

  let showDeleteAllModal = false;
  let isCreating = false;
  let isDeleting = false;

  async function handleCreateChat() {
    isCreating = true;
    try {
      const chat = await createNewChat();
      if (chat) {
        dispatch('chatSelect', { chatId: chat.id });
      }
    } finally {
      isCreating = false;
    }
  }

  function handleDeleteAllClick() {
    showDeleteAllModal = true;
  }

  async function handleDeleteAllConfirm() {
    showDeleteAllModal = false;
    isDeleting = true;
    try {
      await deleteAllChats();
    } finally {
      isDeleting = false;
    }
  }

  function handleDeleteAllCancel() {
    showDeleteAllModal = false;
  }

  function handleChatClick(chatId) {
    dispatch('chatSelect', { chatId });
  }

  function formatTime(dateString) {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="chat-list">
  <div class="chat-list-header">
    <button
      class="{commonStyles['btn']} {commonStyles['btn-primary']} {commonStyles['btn-sm']}"
      disabled={isCreating}
      on:click={handleCreateChat}
    >
      {isCreating ? 'Creating...' : '+ New Chat'}
    </button>
  </div>

  {#if $chatsLoading}
    <div class="chat-list-loading">
      <span class="text-muted">Loading chats...</span>
    </div>
  {:else if $chatsError}
    <div class="chat-list-error">
      <span class="text-error">{$chatsError}</span>
    </div>
  {:else if !$hasChats}
    <div class="chat-list-empty">
      <span class="text-muted">No chats yet</span>
    </div>
  {:else}
    <ul class="{commonStyles['list']} chat-list-items">
      {#each $chats as chat (chat.id)}
        <li
          class="{commonStyles['list-item']} {selectedChatId === chat.id ? commonStyles['active'] : ''}"
          on:click={() => handleChatClick(chat.id)}
          on:keydown={(e) => e.key === 'Enter' && handleChatClick(chat.id)}
          role="button"
          tabindex="0"
          aria-selected={selectedChatId === chat.id}
        >
          <div class="chat-item-content">
            <div class="chat-item-title truncate">{chat.title}</div>
            <div class="chat-item-meta">
              <span class="text-muted text-sm">{formatTime(chat.updatedAt)}</span>
              {#if chat.tags.length > 0}
                <div class="chat-item-tags">
                  {#each chat.tags.slice(0, 2) as tag}
                    <span class={commonStyles['tag']}>{tag}</span>
                  {/each}
                  {#if chat.tags.length > 2}
                    <span class="text-muted text-sm">+{chat.tags.length - 2}</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if $hasChats}
    <div class="chat-list-footer">
      <button
        class="{commonStyles['btn']} {commonStyles['btn-danger']} {commonStyles['btn-sm']}"
        disabled={isDeleting}
        on:click={handleDeleteAllClick}
      >
        {isDeleting ? 'Deleting...' : 'Delete All Chats'}
      </button>
    </div>
  {/if}
</div>

{#if showDeleteAllModal}
  <ConfirmModal
    title="Delete All Chats"
    message="Are you sure you want to delete all chats? This action cannot be undone."
    confirmText="Delete All"
    confirmVariant="danger"
    onConfirm={handleDeleteAllConfirm}
    onCancel={handleDeleteAllCancel}
  />
{/if}

<style>
  .chat-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chat-list-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
  }

  .chat-list-loading,
  .chat-list-error,
  .chat-list-empty {
    padding: var(--spacing-lg);
    text-align: center;
  }

  .chat-list-items {
    flex: 1;
    overflow-y: auto;
  }

  .chat-item-content {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .chat-item-title {
    font-weight: 500;
    font-size: var(--font-size-sm);
  }

  .chat-item-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .chat-item-tags {
    display: flex;
    gap: var(--spacing-xs);
    align-items: center;
  }

  .text-sm {
    font-size: var(--font-size-xs);
  }

  .chat-list-footer {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
  }
</style>
```

### 7. `frontend/src/App.svelte`

**Modify:** Integrate TabView and ChatList components.

```svelte
<script>
  import { onMount } from 'svelte';
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';
  import { initChats } from './lib/stores/chats.js';
  import TabView from './lib/components/TabView.svelte';
  import ChatList from './lib/components/ChatList.svelte';

  /** @type {'chats' | 'plugins'} */
  let activeTab = 'chats';

  /** @type {number | null} */
  let selectedChatId = null;

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

  function handleTabChange(event) {
    activeTab = event.detail.tab;
  }

  function handleChatSelect(event) {
    selectedChatId = event.detail.chatId;
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
          {#if selectedChatId}
            <p>Chat view will be rendered here (Phase 3)</p>
          {:else}
            <div class="empty-state">
              <p class="text-muted">Select a chat or create a new one</p>
            </div>
          {/if}
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

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .plugins-content {
    padding: var(--spacing-lg);
  }
</style>
```

## Tests

### Manual Testing

1. **Chat List Display:**
   - Start chat server
   - Start frontend dev server
   - Verify chat list loads and displays chats
   - Verify chats sorted by updatedAt DESC

2. **Create Chat:**
   - Click "+ New Chat" button
   - Verify new chat appears in list
   - Verify title format: `YYYY-MM-DD HH:mm`
   - Verify new chat is auto-selected

3. **Delete All Chats:**
   - Click "Delete All Chats" button
   - Verify confirmation modal appears
   - Click "Cancel" - verify modal closes, no deletion
   - Click "Delete All" again, then "Delete All" in modal
   - Verify all chats are deleted
   - Verify "Delete All Chats" button is hidden when no chats

4. **Confirmation Modal:**
   - Press Escape key - verify modal closes
   - Click outside modal - verify modal closes
   - Verify modal is accessible (ARIA attributes)

5. **Tab Navigation:**
   - Click "Plugins" tab - verify tab switches
   - Click "Chats" tab - verify tab switches back
   - Verify active tab styling

6. **Real-Time Updates:**
   - Create a chat from another client (e.g., CLI)
   - Verify chat appears in list automatically
   - Delete a chat from another client
   - Verify chat disappears from list automatically

7. **Chat Selection:**
   - Click a chat in the list
   - Verify it becomes selected (highlighted)
   - Verify placeholder content appears in chat view area

## Implementation Notes

1. **Chat Title Format**: Uses `YYYY-MM-DD HH:mm` format as specified. Uses local time, not UTC.

2. **Chat List Sorting**: Sorted by `updatedAt` DESC using derived store. Sorting happens reactively when chats change.

3. **Delete All Implementation**: Deletes chats one by one via API. Each deletion triggers a `chatDeleted` event which updates the store reactively.

4. **CSS Modules**: `common.css` is imported as a CSS module (`?module` suffix) to get scoped class names. This prevents style conflicts while allowing style reuse.

5. **Event Dispatching**: Components use Svelte's `createEventDispatcher` to communicate with parent components. This keeps components decoupled.

6. **Error Handling**: Errors are stored in `chatsError` store and displayed inline in the chat list component.

7. **Loading States**: `chatsLoading` store tracks loading state. Loading indicator shown while fetching initial chat list.

## Dependencies

- **Requires**: Phase 1 (WebSocket foundation, connection store, Zod schemas)
- **Blocks**: Phase 3 (chat view needs chat list to select from)

## Success Criteria

- [ ] Chat list displays all chats from server
- [ ] Chats sorted by updatedAt DESC
- [ ] New chats appear in list immediately after creation
- [ ] Chat titles auto-generated with `YYYY-MM-DD HH:mm` format
- [ ] Delete all chats shows confirmation modal
- [ ] Confirmation modal closes on outside click/Escape
- [ ] Tab navigation switches between Chats and Plugins tabs
- [ ] Chat list updates in real-time when chats are created/deleted by another client
- [ ] Chat selection works (click to select, visual feedback)
- [ ] Empty state shown when no chats exist
- [ ] Loading state shown while fetching chats
- [ ] Error state shown when operations fail
- [ ] CSS modules work correctly (common styles reusable)
- [ ] Delete All button hidden when no chats exist
