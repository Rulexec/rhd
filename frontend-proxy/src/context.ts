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
