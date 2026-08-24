/**
 * Re-export connection store from websocket module.
 * This provides a convenient import path for components.
 */
export { connectionStore } from '../api/websocket.js';
export type { ConnectionState, ConnectionStatus } from '../api/websocket.js';

import type { Readable } from 'svelte/store';
import type { ConnectionState } from '../api/websocket.js';

/**
 * Helper function to check if connected.
 */
export function isConnected(store: Readable<ConnectionState>): boolean {
  let connected = false;
  store.subscribe(({ status }) => {
    connected = status === 'connected';
  })();
  return connected;
}

/**
 * Helper function to get connection error.
 */
export function getConnectionError(store: Readable<ConnectionState>): string | null {
  let error: string | null = null;
  store.subscribe(({ error: err }) => {
    error = err;
  })();
  return error;
}
