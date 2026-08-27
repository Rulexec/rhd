# Frontend Knowledge Base

## Overview

The frontend is a Svelte 5 application (`.svelte.ts`/Svelte runes) using MobX for state management and Svelte's reactivity for UI-only state.

## Architecture

### State Management

- **MobX Stores**: Business logic and application state live in MobX stores under `frontend/src/stores/`
- **Svelte Reactivity**: UI-only state (modals, menus, form inputs) uses Svelte's `$state` rune
- **Bridge**: `mobxObservable()` helper connects MobX observables to Svelte reactivity (see Component Pattern below)

### Store Structure

- `AppStore.ts` - Root store with lazy substore access
- `ConnectionStore.ts` - WebSocket connection state
- `ChatsListStore.ts` - Chat list management
- `ChatStore.ts` - Current chat state and messages
- `PluginsStore.ts` - Plugins list management

### Key Patterns

1. **Dependency Injection**: Stores receive `ChatApi` via constructor for testability
2. **Lazy Initialization**: Substores created on first access via getters
3. **Async Operations**: Use `mobx.flow` (auto-wrapped generator methods) with `yieldPromise` utility
4. **Context Distribution**: AppStore provided via Svelte context

## Store Pattern

Stores are classes using `makeAutoObservable(this)` in the constructor:

- **Async operations** are written as generator **prototype methods** (e.g. `*init()`, `*loadChats()`). `makeAutoObservable` auto-wraps generator methods into flows (actions wrapping a promise chain), so calling `store.loadChats()` returns a `CancellablePromise`, not a suspended generator.
- **`flowResult`**: Callers that want an explicitly-typed promise use `flowResult(...)` — needed when calling a flow method from within another flow (nested flow calls) or from async component code.
- **`yieldPromise`**: The `yieldPromise(promise)` utility (in `frontend/src/util/async.ts`) preserves TypeScript types through the generator, so `const result = yield* yieldPromise(this.#chatApi.getPlugins())` yields a properly typed `result`.
- **Public observable fields (NOT `#`-private)**: State fields that drive reactive getters MUST be public (or underscore-prefixed) observable fields. `#`-private class fields are **not observable by MobX**. Stores therefore store raw state publicly (e.g. `_chats`, `_plugins`, `loading`, `error`) while non-reactive dependencies (`#chatApi`, `#cleanupEvents`) may stay `#`-private.
- **Computed getters**: Derived state is exposed via plain getters (e.g. `chats` sorted, `hasChats`, `activePluginsCount`). `makeAutoObservable` tracks them as computed values.

## Component Pattern (MobX-to-Svelte Bridge)

`mobxObservable` lives in `frontend/src/util/mobxObservable.svelte.ts` — note the **`.svelte.ts`** extension (required so Svelte compiles the runes it contains). It returns a **getter `() => T`**, not a raw value:

```typescript
// In a .svelte component script
const appStore = getAppStore();
const chatsGetter = mobxObservable(() => appStore.chatsList.chats);
```

Usage rules:
1. Call `mobxObservable(() => store.x)` at the **top level** of the component script (it registers `onDestroy` cleanup, so it must run during component init).
2. Bind the result via `$derived`: `let chats = $derived(chatsGetter())`.
3. Use the plain name (`chats`) in the template.

```svelte
<script lang="ts">
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';

  const appStore = getAppStore();
  const chatsGetter = mobxObservable(() => appStore.chatsList.chats);

  let chats = $derived(chatsGetter());
</script>

{#each chats as chat}
  ...
{/each}
```

## Async Operations (Component Side)

Calling a store flow from a component returns a promise thanks to the auto-wrap:

```typescript
import { flowResult } from 'mobx';

async function handleCreate() {
  const chatId = await flowResult(chatsListStore.createNewChat());
  ...
}
```

## File Structure

```
frontend/src/
├── stores/              # MobX stores
│   ├── AppStore.ts
│   ├── ConnectionStore.ts
│   ├── ChatsListStore.ts
│   ├── ChatStore.ts
│   └── PluginsStore.ts
├── util/                # Utilities
│   ├── async.ts         # yieldPromise for typed async generators
│   └── mobxObservable.svelte.ts # MobX-to-Svelte bridge (getter-returning)
├── context.ts           # Svelte context for AppStore (APP_STORE_KEY, setAppStore, getAppStore)
├── main.ts              # Svelte mount entry point
├── lib/
│   ├── api/             # API layer
│   │   ├── ChatApi.ts   # DI interface + defaultChatApi (implementation renamed chatApi.ts → chatApiImpl.ts)
│   │   ├── chatApiImpl.ts # API function implementations
│   │   ├── websocket.ts # WebSocket client
│   │   └── schemas.ts   # Zod schemas and types
│   └── components/      # Svelte components
└── App.svelte           # Root component
```

## AppStore & Context

- `AppStore` (in `frontend/src/stores/AppStore.ts`) is the root store: it receives `{ chatApi }` in its constructor and exposes lazy substore getters (`connection`, `chatsList`, `chat`, `plugins`).
- `frontend/src/context.ts` exports `APP_STORE_KEY` (a `Symbol`) plus `setAppStore(store)` / `getAppStore()`. `App.svelte` creates the store and calls `setAppStore`, children call `getAppStore()`.
- Tests inject a mocked AppStore via Svelte context: `render(Component, { context: new Map([[APP_STORE_KEY, mock]]) })`.

## API Layer

- `frontend/src/lib/api/ChatApi.ts` — the DI interface; functions typed via `typeof chatApiImpl.*`. Also exports `defaultChatApi` (the real implementation). Streaming functions (`streamSubscribe`/`onStreamEvents`) are intentionally NOT part of the interface.
- `frontend/src/lib/api/chatApiImpl.ts` — the concrete API implementation (renamed from `chatApi.ts` during the MobX migration).
- `frontend/src/lib/api/websocket.ts` — WebSocket client; wires lifecycle updates into the `ConnectionStore`.
- `frontend/src/lib/api/schemas.ts` — Zod schemas and derived types.

## Testing

Tests live alongside the code they cover:
- **Store tests**: `frontend/src/stores/*.test.ts` — test stores in isolation with mocked `ChatApi`
- **Component tests**: `frontend/src/lib/components/*.test.ts` — mock AppStore/substores via context render option, verify render/calls

### Store Testing
- Test stores in isolation with mocked `ChatApi`
- Verify state transitions and async flows
- Test event handling

### Component Testing
- Mock AppStore and substores
- Verify components render correct state
- Verify components call correct store methods

## Related Documentation

- [Chat Feature](../features/chat.md)
- [Plugins Feature](../features/plugins.md)
- [Testing](../features/testing.md)