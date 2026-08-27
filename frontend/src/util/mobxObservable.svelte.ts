import { autorun } from 'mobx';
import { onDestroy } from 'svelte';

/**
 * Creates a Svelte $state that stays in sync with a MobX observable.
 *
 * Must be invoked during component initialization, because it registers
 * an `onDestroy` cleanup.
 *
 * Usage in component:
 * ```typescript
 * const store = getAppStore().chat;
 * let messages = mobxObservable(() => store.messages);
 * // Use $messages() in template / $derived
 * ```
 *
 * @param getter - Function that returns the MobX observable value
 * @returns A getter over the Svelte $state that tracks the MobX observable
 */
export function mobxObservable<T>(getter: () => T): () => T {
  let value = $state(getter());

  const dispose = autorun(() => {
    value = getter();
  });

  onDestroy(() => {
    dispose();
  });

  // Return a closure over the $state rune instead of the raw value:
  // returning `value` directly would capture a snapshot and break reactivity.
  return () => value;
}