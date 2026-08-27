import { getContext, setContext } from 'svelte';
import type { AppStore } from './stores/AppStore.js';

/**
 * Svelte context key for the AppStore instance.
 * Exported so tests can provide a mocked AppStore via the context render option
 * (e.g. `render(Component, { context: new Map([[APP_STORE_KEY, mock]]) })`).
 */
export const APP_STORE_KEY = Symbol('appStore');

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