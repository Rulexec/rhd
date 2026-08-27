<script lang="ts">
  import { mobxObservable } from './mobxObservable.svelte.js';

  interface ObservableHarnessStore {
    value: number;
    setValue(nextValue: number): void;
  }

  let { store }: { store: ObservableHarnessStore } = $props();

  const observed = mobxObservable(() => store.value);
</script>

<span data-testid="observed-value">{observed()}</span>