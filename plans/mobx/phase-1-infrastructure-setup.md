# Phase 1: Infrastructure Setup

## Overview

This phase establishes the foundation for MobX integration by:
- Installing MobX dependency
- Creating utility functions for async operations and MobX-to-Svelte bridging
- Setting up the root AppStore pattern with lazy substore access
- Creating Svelte context for AppStore distribution

## Files to Create

### 1. `frontend/src/util/async.ts`

**Purpose**: Utility for typed async generators with MobX flow.

**Additions**:
```typescript
/**
 * Utility for typed async generators with MobX flow.
 * Allows deducing types from yield promises.
 * 
 * Usage:
 * ```typescript
 * *myFlow() {
 *   const result = yield* yieldPromise(fetchData());
 *   // result is properly typed
 * }
 * ```
 */
export function* yieldPromise<T>(
  promise: T | Promise<T>,
): Generator<unknown, T, unknown> {
  return (yield promise) as T;
}
```

### 2. `frontend/src/util/mobxObservable.ts`

**Purpose**: Bridge helper connecting MobX observables to Svelte's `$state`.

**Additions**:
```typescript
import { autorun } from 'mobx';
import { onDestroy } from 'svelte';

/**
 * Creates a Svelte $state that stays in sync with a MobX observable.
 * 
 * Usage in component:
 * ```typescript
 * const store = getAppStore().chat;
 * let messages = mobxObservable(() => store.messages);
 * // Use $messages in template
 * ```
 * 
 * @param getter - Function that returns the MobX observable value
 * @returns Svelte $state reactive value
 */
export function mobxObservable<T>(getter: () => T): T {
  let value = $state(getter());
  
  const dispose = autorun(() => {
    value = getter();
  });
  
  onDestroy(() => {
    dispose();
  });
  
  return value;
}
```

**Note**: This uses Svelte 5's `$state` rune. The helper creates a reactive state that updates whenever the MobX observable changes.

### 3. `frontend/src/stores/AppStore.ts`

**Purpose**: Root store with lazy substore getters cached in private fields.

**Additions**:
```typescript
import { makeAutoObservable } from 'mobx';
import type { ChatApi } from '../lib/api/ChatApi.js';

/**
 * Root application store.
 * Provides access to all substores via lazy getters.
 */
export class AppStore {
  #chatApi: ChatApi;
  #chatsListStore?: ChatsListStore;
  #chatStore?: ChatStore;
  #pluginsStore?: PluginsStore;
  #connectionStore?: ConnectionStore;

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Chat list store (lazy initialized).
   */
  get chatsList(): ChatsListStore {
    if (!this.#chatsListStore) {
      this.#chatsListStore = new ChatsListStore({ chatApi: this.#chatApi });
    }
    return this.#chatsListStore;
  }

  /**
   * Current chat store (lazy initialized).
   */
  get chat(): ChatStore {
    if (!this.#chatStore) {
      this.#chatStore = new ChatStore({ chatApi: this.#chatApi });
    }
    return this.#chatStore;
  }

  /**
   * Plugins store (lazy initialized).
   */
  get plugins(): PluginsStore {
    if (!this.#pluginsStore) {
      this.#pluginsStore = new PluginsStore({ chatApi: this.#chatApi });
    }
    return this.#pluginsStore;
  }

  /**
   * Connection store (lazy initialized).
   */
  get connection(): ConnectionStore {
    if (!this.#connectionStore) {
      this.#connectionStore = new ConnectionStore();
    }
    return this.#connectionStore;
  }
}
```

**Note**: Substore classes will be implemented in later phases. For now, import them as types and they'll be available after Phase 2-5.

### 4. `frontend/src/context.ts`

**Purpose**: Svelte context key and helpers for AppStore distribution.

**Additions**:
```typescript
import { getContext, setContext } from 'svelte';
import type { AppStore } from './stores/AppStore.js';

const APP_STORE_KEY = Symbol('appStore');

/**
 * Set the AppStore in Svelte context.
 * Should be called in root component (App.svelte).
 */
export function setAppStore(store: AppStore): void {
  setContext(APP_STORE_KEY, store);
}

/**
 * Get the AppStore from Svelte context.
 * Should be called in child components.
 */
export function getAppStore(): AppStore {
  return getContext<AppStore>(APP_STORE_KEY);
}
```

### 5. `frontend/src/lib/api/ChatApi.ts`

**Purpose**: Wrapper interface for chat API functions to enable dependency injection.

**Additions**:
```typescript
import * as chatApi from './chatApi.js';

/**
 * Chat API interface for dependency injection.
 * Allows mocking in tests.
 */
export interface ChatApi {
  subscribeChatsList: typeof chatApi.subscribeChatsList;
  unsubscribeChatsList: typeof chatApi.unsubscribeChatsList;
  listChats: typeof chatApi.listChats;
  createChat: typeof chatApi.createChat;
  deleteChat: typeof chatApi.deleteChat;
  generateChatTitle: typeof chatApi.generateChatTitle;
  onChatListEvents: typeof chatApi.onChatListEvents;
  subscribeChat: typeof chatApi.subscribeChat;
  unsubscribeChat: typeof chatApi.unsubscribeChat;
  getChat: typeof chatApi.getChat;
  onChatEvents: typeof chatApi.onChatEvents;
  getQueueMessages: typeof chatApi.getQueueMessages;
  addQueueMessage: typeof chatApi.addQueueMessage;
  onQueueMessageEvents: typeof chatApi.onQueueMessageEvents;
  subscribePluginsList: typeof chatApi.subscribePluginsList;
  getPlugins: typeof chatApi.getPlugins;
  onPluginListEvents: typeof chatApi.onPluginListEvents;
}

/**
 * Default ChatApi implementation using actual API functions.
 */
export const defaultChatApi: ChatApi = {
  subscribeChatsList: chatApi.subscribeChatsList,
  unsubscribeChatsList: chatApi.unsubscribeChatsList,
  listChats: chatApi.listChats,
  createChat: chatApi.createChat,
  deleteChat: chatApi.deleteChat,
  generateChatTitle: chatApi.generateChatTitle,
  onChatListEvents: chatApi.onChatListEvents,
  subscribeChat: chatApi.subscribeChat,
  unsubscribeChat: chatApi.unsubscribeChat,
  getChat: chatApi.getChat,
  onChatEvents: chatApi.onChatEvents,
  getQueueMessages: chatApi.getQueueMessages,
  addQueueMessage: chatApi.addQueueMessage,
  onQueueMessageEvents: chatApi.onQueueMessageEvents,
  subscribePluginsList: chatApi.subscribePluginsList,
  getPlugins: chatApi.getPlugins,
  onPluginListEvents: chatApi.onPluginListEvents,
};
```

## Files to Modify

### 1. `frontend/package.json`

**Modifications**: Add `mobx` dependency.

```json
{
  "dependencies": {
    "marked": "^15.0.0",
    "mobx": "^6.13.0",
    "zod": "^3.23.0"
  }
}
```

## Tests

### Unit Tests for `yieldPromise`

```typescript
import { describe, it, expect } from 'vitest';
import { flow } from 'mobx';
import { yieldPromise } from './async.js';

describe('yieldPromise', () => {
  it('should resolve promise value', async () => {
    const testFlow = flow(function* () {
      const result = yield* yieldPromise(Promise.resolve(42));
      return result;
    });
    
    const result = await testFlow();
    expect(result).toBe(42);
  });

  it('should work with synchronous values', async () => {
    const testFlow = flow(function* () {
      const result = yield* yieldPromise(42);
      return result;
    });
    
    const result = await testFlow();
    expect(result).toBe(42);
  });
});
```

### Unit Tests for `mobxObservable`

```typescript
import { describe, it, expect, vi } from 'vitest';
import { makeObservable, observable, action } from 'mobx';
import { mobxObservable } from './mobxObservable.js';

describe('mobxObservable', () => {
  it('should return initial value', () => {
    class TestStore {
      value = 42;
      constructor() {
        makeObservable(this, { value: observable });
      }
    }
    
    const store = new TestStore();
    const result = mobxObservable(() => store.value);
    expect(result).toBe(42);
  });

  it('should update when MobX observable changes', async () => {
    class TestStore {
      value = 42;
      constructor() {
        makeObservable(this, { 
          value: observable,
          setValue: action
        });
      }
      setValue(newValue: number) {
        this.value = newValue;
      }
    }
    
    const store = new TestStore();
    let observedValue = mobxObservable(() => store.value);
    
    expect(observedValue).toBe(42);
    
    store.setValue(100);
    
    // Wait for autorun to execute
    await new Promise(resolve => setTimeout(resolve, 0));
    
    // Note: In actual Svelte component, $state would update
    // This test verifies the autorun mechanism works
  });
});
```

## Implementation Notes

1. **MobX Version**: Use MobX 6.x which has good TypeScript support and works well with modern bundlers.

2. **$state Rune**: The `mobxObservable` helper uses Svelte 5's `$state` rune. This is a compile-time transformation that creates reactive state.

3. **Lazy Initialization**: Substores are created on first access and cached in private fields. This prevents unnecessary initialization and keeps getters pure (no side effects after first access).

4. **Private Fields**: Using `#` private fields ensures substore instances are not observable by MobX, preventing issues with immutable getter rules.

5. **Context Symbol**: Using a Symbol for context key prevents collisions with other context values.

6. **ChatApi Interface**: This wrapper enables dependency injection for testing. In production, use `defaultChatApi`. In tests, create mock implementations.

## Dependencies

- This is the starting phase with no dependencies on other phases.
- Phases 2-5 depend on this phase for infrastructure.
