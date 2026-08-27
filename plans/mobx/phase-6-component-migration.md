# Phase 6: Component Migration

## Overview

This phase updates all components to use MobX stores via the `mobxObservable` helper. Components will get the AppStore from Svelte context and use `mobxObservable` to bridge MobX observables to Svelte's `$state`. UI-only state (modals, menus, streaming display) remains in Svelte's `$state`.

## Files to Modify

### 1. `frontend/src/App.svelte`

**Purpose**: Initialize AppStore, provide via context, wire up WebSocket to ConnectionStore, replace store imports.

**Changes**:

**Before** (script section):
```typescript
import { onMount } from 'svelte';
import { connectionStore } from './lib/stores/connection.js';
import { websocket } from './lib/api/websocket.js';
import { initChats } from './lib/stores/chats.js';
import { initPlugins } from './lib/stores/plugins.js';
import { selectChat } from './lib/stores/chat.js';
// ... component imports

let activeTab: TabType = $state('chats');
let selectedChatId: number | null = $state(null);

onMount(() => {
  websocket.connect();
  const unsubscribe = connectionStore.subscribe(({ status }) => {
    if (status === 'connected') {
      initChats();
      initPlugins();
      unsubscribe();
    }
  });
});

async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
  selectedChatId = event.detail.chatId;
  await selectChat(selectedChatId);
}
```

**After** (script section):
```typescript
import { onMount } from 'svelte';
import { websocket } from './lib/api/websocket.js';
import { defaultChatApi } from './lib/api/ChatApi.js';
import { AppStore } from './stores/AppStore.js';
import { setAppStore, getAppStore } from './context.js';
import { mobxObservable } from './util/mobxObservable.js';
// ... component imports (same)

// Initialize AppStore
const appStore = new AppStore({ chatApi: defaultChatApi });
setAppStore(appStore);

// Wire WebSocket to ConnectionStore
websocket.setConnectionStore(appStore.connection);

let activeTab: TabType = $state('chats');
let selectedChatId: number | null = $state(null);

// Bridge MobX connection status to Svelte reactivity
let connectionStatus = mobxObservable(() => appStore.connection.status);

onMount(() => {
  websocket.connect();
});

// React to connection status changes
$effect(() => {
  if (connectionStatus === 'connected') {
    appStore.chatsList.init();
    appStore.plugins.init();
  }
});

async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
  selectedChatId = event.detail.chatId;
  await appStore.chat.selectChat(selectedChatId);
}
```

**Template changes**: No template changes needed. The component structure remains the same.

---

### 2. `frontend/src/lib/components/ChatList.svelte`

**Purpose**: Use `mobxObservable` for chats list state instead of Svelte stores.

**Changes**:

**Before** (script section):
```typescript
import { chats, hasChats, chatsLoading, chatsError, createNewChat, deleteAllChats } from '../stores/chats.js';
import { connectionStore } from '../stores/connection.js';
// ...

let showDeleteAllModal: boolean = $state(false);
let isCreating: boolean = $state(false);
let isDeleting: boolean = $state(false);

let isConnected: boolean = $derived($connectionStore.status === 'connected');

async function handleCreateChat() {
  // ...
  const chat = await createNewChat();
  // ...
}

async function handleDeleteAllConfirm() {
  // ...
  const success = await deleteAllChats();
  // ...
}
```

**After** (script section):
```typescript
import { getAppStore } from '../../context.js';
import { mobxObservable } from '../../util/mobxObservable.js';
// ...

const appStore = getAppStore();
const chatsListStore = appStore.chatsList;

// Bridge MobX observables to Svelte $state
let chats = mobxObservable(() => chatsListStore.chats);
let hasChats = mobxObservable(() => chatsListStore.hasChats);
let chatsLoading = mobxObservable(() => chatsListStore.loading);
let chatsError = mobxObservable(() => chatsListStore.error);
let isConnected = mobxObservable(() => appStore.connection.isConnected);

// UI-only state stays in Svelte
let showDeleteAllModal: boolean = $state(false);
let isCreating: boolean = $state(false);
let isDeleting: boolean = $state(false);

async function handleCreateChat() {
  if (!isConnected) return;
  isCreating = true;
  try {
    const chatId = await chatsListStore.createNewChat();
    if (chatId) {
      dispatch('chatSelect', { chatId });
    }
  } catch (error) {
    console.error('Failed to create chat:', error);
  } finally {
    isCreating = false;
  }
}

async function handleDeleteAllConfirm() {
  showDeleteAllModal = false;
  isDeleting = true;
  try {
    const success = await chatsListStore.deleteAllChats();
    if (!success) {
      console.error('Failed to delete all chats');
    }
  } catch (error) {
    console.error('Failed to delete all chats:', error);
  } finally {
    isDeleting = false;
  }
}
```

**Template changes**: Replace `$chats` → `$chats`, `$chatsLoading` → `$chatsLoading`, etc. (same variable names, but now backed by `mobxObservable`). Replace `chatsError.set(null)` → `chatsListStore.error = null` (or add a `clearError` method).

**Note**: The `handleDeleteAllClick` and other UI handlers remain the same. The `formatTime` function remains unchanged.

---

### 3. `frontend/src/lib/components/ChatView.svelte`

**Purpose**: Use `mobxObservable` for current chat state. Keep streaming display logic as component-local state.

**Changes**:

**Before** (script section):
```typescript
import { onMount, onDestroy } from 'svelte';
import { currentChat, allMessages, chatLoading, chatError } from '../stores/chat.js';
import { streamSubscribe, onStreamEvents } from '../api/chatApi.js';
// ...

function dismissError() {
  chatError.set(null);
}
```

**After** (script section):
```typescript
import { onMount, onDestroy } from 'svelte';
import { getAppStore } from '../../context.js';
import { mobxObservable } from '../../util/mobxObservable.js';
import { streamSubscribe, onStreamEvents } from '../api/chatApi.js';
// ...

const appStore = getAppStore();
const chatStore = appStore.chat;

// Bridge MobX observables to Svelte $state
let currentChat = mobxObservable(() => chatStore.currentChat);
let allMessages = mobxObservable(() => chatStore.allMessages);
let chatLoading = mobxObservable(() => chatStore.loading);
let chatError = mobxObservable(() => chatStore.error);

function dismissError() {
  chatStore.error = null;
}
```

**Template changes**: Replace `$currentChat` → `$currentChat`, `$allMessages` → `$allMessages`, etc. (same variable names, now backed by `mobxObservable`).

**Note**: Streaming display logic (`streamSubscriptions`, `ensureStreamSubscription`, `getStreamContent`) remains as component-local `$state` since it's UI-specific presentation logic.

---

### 4. `frontend/src/lib/components/Message.svelte`

**Purpose**: No changes needed. This is a pure presentational component that receives data via props.

**Changes**: None.

---

### 5. `frontend/src/lib/components/MessageInput.svelte`

**Purpose**: Use `mobxObservable` for chat state and connection state. Move `addQueueMessage` call to store.

**Changes**:

**Before** (script section):
```typescript
import { currentChatId } from '../stores/chat.js';
import { connectionStore } from '../stores/connection.js';
import { addQueueMessage } from '../api/chatApi.js';

let isConnected: boolean = $derived($connectionStore.status === 'connected');
let canSend: boolean = $derived(
  inputValue.trim().length > 0 && $currentChatId != null && isConnected && !isSending
);

async function handleSend(): Promise<void> {
  if (!canSend || $currentChatId == null) return;
  // ...
  await addQueueMessage($currentChatId, 'user', content);
  // ...
}
```

**After** (script section):
```typescript
import { getAppStore } from '../../context.js';
import { mobxObservable } from '../../util/mobxObservable.js';

const appStore = getAppStore();
const chatStore = appStore.chat;

// Bridge MobX observables to Svelte $state
let currentChatId = mobxObservable(() => chatStore.currentChatId);
let isConnected = mobxObservable(() => appStore.connection.isConnected);

// UI-only state stays in Svelte
let inputValue: string = $state('');
let textareaEl: HTMLTextAreaElement | null = $state(null);
let isSending: boolean = $state(false);
let errorMessage: string | null = $state(null);

let canSend: boolean = $derived(
  inputValue.trim().length > 0 && $currentChatId != null && $isConnected && !isSending
);

async function handleSend(): Promise<void> {
  if (!canSend || $currentChatId == null) return;
  // ...
  await appStore.chat.addQueueMessage($currentChatId, 'user', content);
  // ...
}
```

**Note**: The `addQueueMessage` method needs to be added to `ChatStore` (see implementation notes below).

**Template changes**: Replace `$currentChatId` → `$currentChatId`, `$connectionStore.status` → `$isConnected` (already derived).

---

### 6. `frontend/src/lib/components/PluginList.svelte`

**Purpose**: Use `mobxObservable` for plugins state.

**Changes**:

**Before** (script section):
```typescript
import { plugins, hasPlugins, pluginsLoading, pluginsError, activePluginsCount } from '../stores/plugins.js';
```

**After** (script section):
```typescript
import { getAppStore } from '../../context.js';
import { mobxObservable } from '../../util/mobxObservable.js';

const appStore = getAppStore();
const pluginsStore = appStore.plugins;

let plugins = mobxObservable(() => pluginsStore.plugins);
let hasPlugins = mobxObservable(() => pluginsStore.hasPlugins);
let pluginsLoading = mobxObservable(() => pluginsStore.loading);
let pluginsError = mobxObservable(() => pluginsStore.error);
let activePluginsCount = mobxObservable(() => pluginsStore.activePluginsCount);
```

**Template changes**: Replace `$plugins` → `$plugins`, `$hasPlugins` → `$hasPlugins`, etc. (same variable names, now backed by `mobxObservable`).

---

### 7. `frontend/src/lib/components/ConnectionStatus.svelte`

**Purpose**: Use `mobxObservable` for connection state.

**Changes**:

**Before** (script section):
```typescript
import { connectionStore } from '../stores/connection.js';
import { websocket } from '../api/websocket.js';

let status = $derived($connectionStore.status);
let error = $derived($connectionStore.error);
let isConnected = $derived(status === 'connected');
```

**After** (script section):
```typescript
import { getAppStore } from '../../context.js';
import { mobxObservable } from '../../util/mobxObservable.js';
import { websocket } from '../api/websocket.js';

const appStore = getAppStore();
const connectionStore = appStore.connection;

let status = mobxObservable(() => connectionStore.status);
let error = mobxObservable(() => connectionStore.error);
let isConnected = $derived($status === 'connected');
let isDisconnected = $derived($status === 'disconnected');
let isConnecting = $derived($status === 'connecting');
```

**Template changes**: No template changes needed. The `$status`, `$error` variables work the same way.

---

### 8. `frontend/src/lib/components/TabView.svelte`

**Purpose**: No changes needed. Pure UI component with no store dependencies.

**Changes**: None.

---

### 9. `frontend/src/lib/components/ConfirmModal.svelte`

**Purpose**: No changes needed. Pure UI component with no store dependencies.

**Changes**: None.

---

## Additional Store Changes

### `frontend/src/stores/ChatStore.ts`

**Addition**: Add `addQueueMessage` method to ChatStore.

```typescript
/**
 * Add a message to the queue.
 */
*addQueueMessage(chatId: number, role: string, content: string, tags: string[] = []): Generator {
  try {
    yield* yieldPromise(this.#chatApi.addQueueMessage(chatId, role, content, tags));
  } catch (error) {
    this.error = error instanceof Error ? error.message : String(error);
    throw error;
  }
}
```

## Tests

### Component Tests (Example: ChatList)

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/svelte';
import ChatList from './ChatList.svelte';
import { AppStore } from '../stores/AppStore.js';
import { setAppStore } from '../context.js';

describe('ChatList', () => {
  it('should render empty state when no chats', () => {
    const mockStore = createMockAppStore({ chats: [] });
    // Set up context...
    
    const { getByText } = render(ChatList);
    expect(getByText('No chats yet')).toBeTruthy();
  });

  it('should render chat list', () => {
    const mockStore = createMockAppStore({ 
      chats: [{ id: 1, title: 'Test Chat', ... }] 
    });
    
    const { getByText } = render(ChatList);
    expect(getByText('Test Chat')).toBeTruthy();
  });

  it('should call createNewChat on button click', async () => {
    const mockStore = createMockAppStore();
    const createSpy = vi.spyOn(mockStore.chatsList, 'createNewChat');
    
    const { getByText } = render(ChatList);
    await fireEvent.click(getByText('+ New Chat'));
    
    expect(createSpy).toHaveBeenCalled();
  });
});
```

### Component Tests (Example: ConnectionStatus)

```typescript
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import ConnectionStatus from './ConnectionStatus.svelte';

describe('ConnectionStatus', () => {
  it('should not render when connected', () => {
    const mockStore = createMockAppStore({ connectionStatus: 'connected' });
    
    const { queryByText } = render(ConnectionStatus);
    expect(queryByText('Connected')).toBeNull();
  });

  it('should render disconnected status', () => {
    const mockStore = createMockAppStore({ connectionStatus: 'disconnected' });
    
    const { getByText } = render(ConnectionStatus);
    expect(getByText('Disconnected')).toBeTruthy();
  });

  it('should call websocket.disconnect and connect on reconnect', async () => {
    const mockStore = createMockAppStore({ connectionStatus: 'disconnected' });
    
    const { getByText } = render(ConnectionStatus);
    await fireEvent.click(getByText('Reconnect'));
    
    // Verify websocket methods called
  });
});
```

## Implementation Notes

1. **mobxObservable Usage**: Each component creates local variables backed by `mobxObservable`. These variables use the `$` prefix in templates just like Svelte store subscriptions.

2. **Store Access Pattern**: Components get the AppStore via `getAppStore()` and access substores through it. This avoids prop drilling.

3. **UI State Separation**: Component-local state (modals, form inputs, streaming display) remains in Svelte's `$state`. Only data from MobX stores uses `mobxObservable`.

4. **Method Calls**: Components call store methods directly (e.g., `chatsListStore.createNewChat()`) instead of imported functions.

5. **Error Clearing**: Instead of `chatError.set(null)`, components set `chatStore.error = null` directly or call a `clearError()` method.

6. **Streaming Logic**: The streaming display logic in ChatView remains as component-local state because it's about visual presentation of streaming content, not business logic.

7. **Import Path Changes**: Components now import from `../../context.js` and `../../util/mobxObservable.js` instead of `../stores/*.js`.

8. **Backward Compatibility**: During migration, old store files still exist. Components are migrated one by one. After all components are migrated, old stores are removed in Phase 7.

## Dependencies

- Depends on Phases 2, 3, 4, 5 (all stores must be implemented).
- Must be completed before Phase 7 (Cleanup).
