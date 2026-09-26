# Phase 3: API Client + MobX Store Layer

## Overview

Build the browser-side data layer of the viewer:

1. **`ProxyLogsApi`** — a fetch-based client for the three Phase 2 endpoints, validating
   responses with the shared Zod contract, injectable as an interface for tests (same DI pattern
   as `frontend/src/lib/api/ChatApi.ts`).
2. **`ProxyLogsStore`** — a single MobX root store holding the chats list, the selected chat
   detail, the selected request detail, loading/error state per area, and the flows
   `loadChats` / `openChat` / `openRequest` / `closeRequest` / `refresh`. The **refresh button
   semantics** (Phase 4's button calls `refresh()`) are defined here: re-fetch everything visible
   while **preserving the current selection**.
3. Svelte context plumbing (`context.ts`) and the two util files copied from `frontend/`
   (`mobxObservable.svelte.ts`, `async.ts`) that later components need.

No UI in this phase — everything is unit-tested against a mocked API.

**Conventions followed (from `memory/frontend/MEMORY.md`, verified in
`frontend/src/stores/ChatsListStore.ts`):** `makeAutoObservable` classes; async operations as
generator **prototype methods** (auto-wrapped into flows); state that drives reactive getters in
**public** fields (`#`-private fields are not observable); non-reactive dependencies stay
`#`-private; typed yields via `yield* yieldPromise(...)`; error messages via
`error instanceof Error ? error.message : String(error)`.

## Files to Create

### 1. `frontend-proxy/src/util/mobxObservable.svelte.ts`

Exact copy of `frontend/src/util/mobxObservable.svelte.ts` (the MobX→Svelte bridge; the
`.svelte.ts` extension is required so Svelte compiles its runes):

```typescript
import { autorun } from 'mobx';
import { onDestroy } from 'svelte';

/**
 * Creates a Svelte $state that stays in sync with a MobX observable.
 * Must be invoked during component initialization (registers onDestroy cleanup).
 */
export function mobxObservable<T>(getter: () => T): () => T {
  let value = $state(getter());

  const dispose = autorun(() => {
    value = getter();
  });

  onDestroy(() => {
    dispose();
  });

  return () => value;
}
```

### 2. `frontend-proxy/src/util/async.ts`

Exact copy of `frontend/src/util/async.ts`:

```typescript
/**
 * Utility for typed async generators with MobX flow.
 * Usage:
 * ```typescript
 * *myFlow() {
 *   const result = yield* yieldPromise(fetchData());
 * }
 * ```
 */
export function* yieldPromise<T>(
  promise: T | Promise<T>,
): Generator<unknown, T, unknown> {
  return (yield promise) as T;
}
```

### 3. `frontend-proxy/src/lib/api/proxyLogsApiImpl.ts` — concrete client

```typescript
import type { ZodType } from 'zod';
import {
  ApiErrorSchema,
  ChatDetailSchema,
  ChatsResponseSchema,
  RequestDetailSchema
} from './schemas.js';
import type { ChatDetail, ChatsResponse, RequestDetail } from './schemas.js';

/** API failure with the HTTP status (0 = network/dev-server unreachable). */
export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

async function parseErrorMessage(response: Response): Promise<string> {
  try {
    const body: unknown = await response.json();
    const parsed = ApiErrorSchema.safeParse(body);
    if (parsed.success) {
      return parsed.data.error;
    }
  } catch {
    // Not a JSON error envelope — fall through.
  }
  return `request failed with status ${response.status}`;
}

async function fetchJson<T>(url: string, schema: ZodType<T>): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (cause) {
    throw new ApiError(
      `dev server unreachable: ${cause instanceof Error ? cause.message : String(cause)}`,
      0
    );
  }
  if (!response.ok) {
    throw new ApiError(await parseErrorMessage(response), response.status);
  }
  const data: unknown = await response.json();
  return schema.parse(data);
}

export function listChats(): Promise<ChatsResponse> {
  return fetchJson('/api/chats', ChatsResponseSchema);
}

export function getChatDetail(chatId: number): Promise<ChatDetail> {
  return fetchJson(`/api/chats/${chatId}`, ChatDetailSchema);
}

export function getRequestDetail(requestId: number): Promise<RequestDetail> {
  return fetchJson(`/api/requests/${requestId}`, RequestDetailSchema);
}
```

### 4. `frontend-proxy/src/lib/api/ProxyLogsApi.ts` — DI interface + default

Mirrors `frontend/src/lib/api/ChatApi.ts` (interface via `typeof impl.fn`, default object binds
the real implementation):

```typescript
import * as proxyLogsApi from './proxyLogsApiImpl.js';

/**
 * Proxy logs API interface for dependency injection. Allows mocking in tests.
 */
export interface ProxyLogsApi {
  listChats: typeof proxyLogsApi.listChats;
  getChatDetail: typeof proxyLogsApi.getChatDetail;
  getRequestDetail: typeof proxyLogsApi.getRequestDetail;
}

/** Default implementation backed by the Vite dev-server middleware (Phase 2). */
export const defaultProxyLogsApi: ProxyLogsApi = {
  listChats: proxyLogsApi.listChats,
  getChatDetail: proxyLogsApi.getChatDetail,
  getRequestDetail: proxyLogsApi.getRequestDetail
};

export { ApiError } from './proxyLogsApiImpl.js';
```

### 5. `frontend-proxy/src/stores/ProxyLogsStore.ts` — the root store

One store (no substores): the app has a single screen of state, unlike the product frontend's
multi-domain root. Field/method names below are the contract Phases 4–5 render against.

```typescript
import { makeAutoObservable } from 'mobx';
import { ApiError } from '../lib/api/ProxyLogsApi.js';
import type { ProxyLogsApi } from '../lib/api/ProxyLogsApi.js';
import type { ChatSummary } from '../lib/api/schemas.js';
import { yieldPromise } from '../util/async.js';

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Root store of the proxy-logs viewer.
 *
 * Async operations are generator prototype methods (auto-wrapped into flows by
 * makeAutoObservable — calling store.loadChats() returns a CancellablePromise
 * awaitable directly; use flowResult(...) when an explicitly-typed promise is
 * wanted, e.g. from components). Reactive state is stored in public fields.
 */
export class ProxyLogsStore {
  #api: ProxyLogsApi;

  // ---- chats list area -------------------------------------------------
  /** Raw chat list as served (already updated_at DESC). Public for MobX. */
  _chats: ChatSummary[] = [];
  chatsLoading: boolean = false;
  chatsError: string | null = null;

  // ---- chat detail area ------------------------------------------------
  selectedChatId: number | null = null;
  chatDetail: import('../lib/api/schemas.js').ChatDetail | null = null;
  detailLoading: boolean = false;
  detailError: string | null = null;

  // ---- request detail area ---------------------------------------------
  selectedRequestId: number | null = null;
  requestDetail: import('../lib/api/schemas.js').RequestDetail | null = null;

  // ---- refresh ----------------------------------------------------------
  refreshing: boolean = false;

  constructor(options: { api: ProxyLogsApi }) {
    this.#api = options.api;
    makeAutoObservable(this);
  }

  get chats(): ChatSummary[] {
    return this._chats;
  }

  get hasChats(): boolean {
    return this._chats.length > 0;
  }

  /** Summary of the selected chat, or null when it is no longer in the list. */
  get selectedChat(): ChatSummary | null {
    return this._chats.find((chat) => chat.id === this.selectedChatId) ?? null;
  }

  /** Load the chat list (initial load; the refresh button uses refresh()). */
  *loadChats(): Generator<unknown, void, unknown> {
    this.chatsLoading = true;
    this.chatsError = null;
    try {
      const result = yield* yieldPromise(this.#api.listChats());
      this._chats = result.chats;
    } catch (error) {
      this.chatsError = errorMessage(error);
    } finally {
      this.chatsLoading = false;
    }
  }

  /** Open a chat: set selection, drop any open request, fetch its detail. */
  *openChat(chatId: number): Generator<unknown, void, unknown> {
    this.selectedChatId = chatId;
    this.selectedRequestId = null;
    this.requestDetail = null;
    this.chatDetail = null;
    this.detailError = null;
    this.detailLoading = true;
    try {
      const detail = yield* yieldPromise(this.#api.getChatDetail(chatId));
      this.chatDetail = detail;
    } catch (error) {
      this.detailError = errorMessage(error);
    } finally {
      this.detailLoading = false;
    }
  }

  /** Open one request's raw detail (timeline row click). */
  *openRequest(requestId: number): Generator<unknown, void, unknown> {
    this.selectedRequestId = requestId;
    this.requestDetail = null;
    try {
      const detail = yield* yieldPromise(this.#api.getRequestDetail(requestId));
      this.requestDetail = detail;
    } catch (error) {
      // Surface in the detail area; the timeline stays usable.
      this.detailError = errorMessage(error);
    }
  }

  /** Close the request drill-down (does not touch the chat selection). */
  closeRequest(): void {
    this.selectedRequestId = null;
    this.requestDetail = null;
  }

  /**
   * Refresh button: re-fetch everything currently visible, preserving the
   * selection. A selected chat that no longer exists after the refresh clears
   * the selection (same for a selected request missing from the refreshed
   * chat detail).
   */
  *refresh(): Generator<unknown, void, unknown> {
    this.refreshing = true;
    this.chatsError = null;
    try {
      const result = yield* yieldPromise(this.#api.listChats());
      this._chats = result.chats;

      if (this.selectedChatId !== null) {
        const stillExists = this._chats.some((chat) => chat.id === this.selectedChatId);
        if (!stillExists) {
          this.clearSelection();
        } else {
          const detail = yield* yieldPromise(this.#api.getChatDetail(this.selectedChatId));
          this.chatDetail = detail;
          this.detailError = null;
        }
      }

      if (this.selectedRequestId !== null) {
        const stillPresent =
          this.chatDetail?.requests.some((r) => r.id === this.selectedRequestId) ?? false;
        if (!stillPresent) {
          this.closeRequest();
        } else {
          this.requestDetail = yield* yieldPromise(
            this.#api.getRequestDetail(this.selectedRequestId)
          );
        }
      }
    } catch (error) {
      // A 404 mid-refresh means the selection vanished server-side; treat as
      // a selection change, not an error banner.
      if (error instanceof ApiError && error.status === 404) {
        this.clearSelection();
      } else {
        this.chatsError = errorMessage(error);
      }
    } finally {
      this.refreshing = false;
    }
  }

  /** Drop chat + request selection and related detail state. */
  clearSelection(): void {
    this.selectedChatId = null;
    this.chatDetail = null;
    this.detailError = null;
    this.closeRequest();
  }
}
```

**Style note:** replace the two `import('../lib/api/schemas.js')` inline type imports with normal
top-level `import type { ChatDetail, RequestDetail }` — shown inline above only to keep the
snippet copy-paste-safe about import ordering; the real file uses the top-level imports.

### 6. `frontend-proxy/src/context.ts`

Mirror of `frontend/src/context.ts`:

```typescript
import { getContext, setContext } from 'svelte';
import type { ProxyLogsStore } from './stores/ProxyLogsStore.js';

/**
 * Svelte context key for the ProxyLogsStore instance. Exported so tests can
 * inject a mocked store: render(Component, { context: new Map([[PROXY_LOGS_STORE_KEY, mock]]) }).
 */
export const PROXY_LOGS_STORE_KEY = Symbol('proxyLogsStore');

export function setProxyLogsStore(store: ProxyLogsStore): void {
  setContext(PROXY_LOGS_STORE_KEY, store);
}

export function getProxyLogsStore(): ProxyLogsStore {
  return getContext<ProxyLogsStore>(PROXY_LOGS_STORE_KEY);
}
```

### 7. `frontend-proxy/src/stores/ProxyLogsStore.test.ts` — store tests

Follow the structure of `frontend/src/stores/ChatsListStore.test.ts`: build a `mockApi` object
with `vi.fn()`s cast to `ProxyLogsApi`, construct the real store, await flows directly (they
return CancellablePromises).

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ProxyLogsStore } from './ProxyLogsStore.js';
import { ApiError } from '../lib/api/ProxyLogsApi.js';
import type { ProxyLogsApi } from '../lib/api/ProxyLogsApi.js';
import type { ChatDetail, ChatSummary, RequestDetail } from '../lib/api/schemas.js';
```

Shared fixtures (define once at the top): a `chatSummary(id)` helper, a `chatDetail(chatId,
requestIds)` builder including a small `conversation` array, and a `requestDetail(id)` builder.

**Test cases (write all of these):**

1. Initial state: `chats === []`, `hasChats === false`, `selectedChatId === null`,
   `chatDetail === null`, `requestDetail === null`, all loading/error flags false/null.
2. `loadChats` success: `_chats` populated, `chatsLoading` false after, `chatsError` null.
3. `loadChats` failure (`mockRejectedValueOnce(new Error('boom'))`): `chatsError === 'boom'`,
   loading flag reset, list unchanged.
4. `openChat` success: sets `selectedChatId`, populates `chatDetail`, clears
   `detailLoading`; `selectedChat` getter returns the right summary.
5. `openChat` while a request is open: `selectedRequestId`/`requestDetail` reset to null.
6. `openChat` failure: `detailError` set, `chatDetail` stays null.
7. `openRequest` success: sets `selectedRequestId`, populates `requestDetail`.
8. `openRequest` failure: `detailError` set.
9. `closeRequest`: clears `requestDetail` + `selectedRequestId`, keeps chat selection.
10. `refresh` with nothing selected: only `listChats` called; `refreshing` toggled true→false.
11. `refresh` preserving selection: seed chats [1, 2], `openChat(1)`, `openRequest(10)`; mock
    refreshed data including the same chat + request; call `refresh`; assert `listChats`,
    `getChatDetail`, `getRequestDetail` all re-called, `selectedChatId === 1`,
    `selectedRequestId === 10`, new data in `chatDetail`/`requestDetail`.
12. `refresh` when the selected chat disappeared: refreshed chats lack id 1 → `clearSelection`
    ran (`selectedChatId === null`, `chatDetail === null`).
13. `refresh` when the selected request disappeared from the refreshed detail:
    `selectedRequestId === null`, `requestDetail === null`, but chat selection kept.
14. `refresh` with `getChatDetail` rejecting `new ApiError('chat not found', 404)` → selection
    cleared, **no** `chatsError`.
15. `refresh` with a non-404 rejection → `chatsError` set.
16. `clearSelection` resets everything selection-related.

## Implementation Notes

1. **Flows return CancellablePromise:** tests can `await store.loadChats()` directly (see the
   note in `ChatsListStore.test.ts` / `mobx-probe.test.ts` semantics). Components in Phases 4–5
   use `flowResult(...)` when they need a typed promise — matching the frontend convention.
2. **Why one store, not substores:** the product frontend splits by domain (connection, chats
   list, chat, plugins) because each has events/subscriptions. Here there is one fetch-only
   domain; a single class keeps the refresh-preserves-selection logic in one place instead of
   spread across coordination code.
3. **`ApiError` re-export:** the store imports `ApiError` from `ProxyLogsApi.js` (the interface
   module), keeping implementation details (`proxyLogsApiImpl`) invisible to consumers — same
   layering as the frontend's `ChatApi.ts`.
4. **404-during-refresh semantics:** deletion is impossible from this viewer, but the logging DB
   can be reset externally (`rm chats.sqlite3`); treating vanished selections as selection
   changes avoids a scary error banner after an intentional DB reset.
5. **No Zod parse of requests in the store:** parsing already happened in the API layer; the
   store deals in typed data only.
6. **Zod v3 `ZodType<T>`:** the API layer types the schema parameter as `ZodType<T>` so
   `fetchJson` infers the response type from the schema argument.

## Dependencies

- **Depends on:** Phase 2 — `src/lib/api/schemas.ts` (the contract) and the live endpoints for
  manual verification. The mocked-API tests only need the schemas, so writing tests can start as
  soon as Phase 2's `schemas.ts` review is done, but this phase completes after Phase 2.
- **Blocks:** Phase 4 (components render this store), Phase 5 (same).
- **Parallel with:** nothing; Phase 4's leaf components can be sketched but not tested without
  this store.

## Verification Summary

```sh
npm --prefix frontend-proxy run test    # ProxyLogsStore.test.ts green
npm --prefix frontend-proxy run check   # svelte-check green
# optional end-to-end sanity with the real API (Phase 2 server running):
VITE_PROXY_LOGS_PATH=<dir> npm --prefix frontend-proxy run dev
# then in another shell: curl -s localhost:5174/api/chats
```
