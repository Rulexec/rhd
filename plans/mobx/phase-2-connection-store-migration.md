# Phase 2: ConnectionStore Migration

## Overview

This phase migrates WebSocket connection state from Svelte store to MobX store. The ConnectionStore will manage connection status, error state, and provide methods for connection lifecycle.

## Files to Create

### 1. `frontend/src/stores/ConnectionStore.ts`

**Purpose**: MobX store for WebSocket connection state.

**Additions**:
```typescript
import { makeAutoObservable } from 'mobx';

export type ConnectionStatus = 'connected' | 'disconnected' | 'connecting';

export interface ConnectionState {
  status: ConnectionStatus;
  error: string | null;
}

/**
 * MobX store for WebSocket connection state.
 */
export class ConnectionStore {
  status: ConnectionStatus = 'disconnected';
  error: string | null = null;

  constructor() {
    makeAutoObservable(this);
  }

  /**
   * Set connection status to connecting.
   */
  setConnecting(): void {
    this.status = 'connecting';
    this.error = null;
  }

  /**
   * Set connection status to connected.
   */
  setConnected(): void {
    this.status = 'connected';
    this.error = null;
  }

  /**
   * Set connection status to disconnected with optional error.
   */
  setDisconnected(error: string | null = null): void {
    this.status = 'disconnected';
    this.error = error;
  }

  /**
   * Check if currently connected.
   */
  get isConnected(): boolean {
    return this.status === 'connected';
  }

  /**
   * Clear any error state.
   */
  clearError(): void {
    this.error = null;
  }
}
```

## Files to Modify

### 1. `frontend/src/lib/api/websocket.ts`

**Modifications**: Update WebSocketClient to use ConnectionStore instead of internal Svelte store.

**Changes**:
1. Remove the `connectionStore` Svelte store export
2. Accept ConnectionStore in constructor or via setter
3. Update all `connectionStore.set()` calls to use ConnectionStore methods

**Before**:
```typescript
import { writable, type Writable } from 'svelte/store';

export const connectionStore: Writable<ConnectionState> = writable({
  status: 'disconnected',
  error: null
});

class WebSocketClient {
  connect(): void {
    connectionStore.set({ status: 'connecting', error: null });
    // ...
  }
}
```

**After**:
```typescript
import type { ConnectionStore } from '../stores/ConnectionStore.js';

class WebSocketClient {
  private connectionStore: ConnectionStore | null = null;

  /**
   * Set the connection store for state updates.
   */
  setConnectionStore(store: ConnectionStore): void {
    this.connectionStore = store;
  }

  connect(): void {
    this.connectionStore?.setConnecting();
    // ...
  }

  // Update all other connectionStore.set() calls:
  // - connectionStore.set({ status: 'connected', error: null }) 
  //   → this.connectionStore?.setConnected()
  // - connectionStore.set({ status: 'disconnected', error: errorMessage })
  //   → this.connectionStore?.setDisconnected(errorMessage)
}

export const websocket = new WebSocketClient();
```

**Specific changes in WebSocketClient**:
- Line 47: `connectionStore.set({ status: 'connecting', error: null })` → `this.connectionStore?.setConnecting()`
- Line 56: `connectionStore.set({ status: 'connected', error: null })` → `this.connectionStore?.setConnected()`
- Line 73: `connectionStore.set({ status: 'disconnected', error: errorMessage })` → `this.connectionStore?.setDisconnected(errorMessage)`
- Line 79: `connectionStore.set({ status: 'disconnected', error: 'Connection error occurred' })` → `this.connectionStore?.setDisconnected('Connection error occurred')`
- Line 88: `connectionStore.set({ status: 'disconnected', error: this.errorMessage })` → `this.connectionStore?.setDisconnected(this.errorMessage)`
- Line 102: `connectionStore.set({ status: 'disconnected', error: null })` → `this.connectionStore?.setDisconnected(null)`

### 2. `frontend/src/lib/stores/connection.ts`

**Modifications**: Re-export from new ConnectionStore for backward compatibility during migration.

**Changes**: Replace entire file content with re-exports.

**Before**:
```typescript
export { connectionStore } from '../api/websocket.js';
export type { ConnectionState, ConnectionStatus } from '../api/websocket.js';

import type { Readable } from 'svelte/store';
import type { ConnectionState } from '../api/websocket.js';

export function isConnected(store: Readable<ConnectionState>): boolean {
  // ...
}

export function getConnectionError(store: Readable<ConnectionState>): string | null {
  // ...
}
```

**After**:
```typescript
/**
 * @deprecated Use ConnectionStore from stores/ConnectionStore.ts instead.
 * This file is kept for backward compatibility during migration.
 */
export { ConnectionStore, type ConnectionState, type ConnectionStatus } from '../stores/ConnectionStore.js';
```

## Tests

### Unit Tests for ConnectionStore

```typescript
import { describe, it, expect } from 'vitest';
import { ConnectionStore } from './ConnectionStore.js';

describe('ConnectionStore', () => {
  it('should initialize with disconnected status', () => {
    const store = new ConnectionStore();
    expect(store.status).toBe('disconnected');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(false);
  });

  it('should set connecting status', () => {
    const store = new ConnectionStore();
    store.setConnecting();
    expect(store.status).toBe('connecting');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(false);
  });

  it('should set connected status', () => {
    const store = new ConnectionStore();
    store.setConnected();
    expect(store.status).toBe('connected');
    expect(store.error).toBe(null);
    expect(store.isConnected).toBe(true);
  });

  it('should set disconnected status with error', () => {
    const store = new ConnectionStore();
    store.setDisconnected('Connection lost');
    expect(store.status).toBe('disconnected');
    expect(store.error).toBe('Connection lost');
    expect(store.isConnected).toBe(false);
  });

  it('should clear error', () => {
    const store = new ConnectionStore();
    store.setDisconnected('Some error');
    expect(store.error).toBe('Some error');
    
    store.clearError();
    expect(store.error).toBe(null);
    expect(store.status).toBe('disconnected');
  });
});
```

### Integration Tests for WebSocket with ConnectionStore

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { websocket } from './websocket.js';
import { ConnectionStore } from '../stores/ConnectionStore.js';

describe('WebSocket with ConnectionStore', () => {
  let connectionStore: ConnectionStore;

  beforeEach(() => {
    connectionStore = new ConnectionStore();
    websocket.setConnectionStore(connectionStore);
  });

  it('should update connection store on connect', () => {
    expect(connectionStore.status).toBe('disconnected');
    
    // Mock WebSocket
    const mockWs = {
      readyState: WebSocket.CONNECTING,
      onopen: null,
      onclose: null,
      onerror: null,
      onmessage: null,
      send: vi.fn(),
      close: vi.fn()
    };
    vi.spyOn(global, 'WebSocket').mockImplementation(() => mockWs as any);
    
    websocket.connect();
    
    expect(connectionStore.status).toBe('connecting');
    
    // Simulate connection open
    mockWs.readyState = WebSocket.OPEN;
    mockWs.onopen?.({} as Event);
    
    expect(connectionStore.status).toBe('connected');
    expect(connectionStore.isConnected).toBe(true);
  });

  it('should update connection store on disconnect', () => {
    connectionStore.setConnected();
    expect(connectionStore.isConnected).toBe(true);
    
    websocket.disconnect();
    
    expect(connectionStore.status).toBe('disconnected');
    expect(connectionStore.isConnected).toBe(false);
  });
});
```

## Implementation Notes

1. **Backward Compatibility**: The old `connection.ts` file is kept as a re-export to avoid breaking existing imports during migration. It will be removed in Phase 7.

2. **Optional Store**: The WebSocketClient uses optional chaining (`this.connectionStore?.`) because the store might not be set immediately. This allows the WebSocket to function even if the store is not configured.

3. **Method-Based Updates**: Instead of directly setting state objects, ConnectionStore provides methods (`setConnecting`, `setConnected`, `setDisconnected`) that encapsulate state transitions. This makes the code more maintainable and ensures consistent state updates.

4. **Computed Property**: `isConnected` is a getter that derives from `status`, making it automatically reactive in MobX.

5. **No Event Listeners**: ConnectionStore doesn't manage event listeners directly. The WebSocketClient handles connection events and calls store methods accordingly.

## Dependencies

- Depends on Phase 1 for infrastructure (MobX, utilities).
- Must be completed before Phase 6 (Component Migration).
