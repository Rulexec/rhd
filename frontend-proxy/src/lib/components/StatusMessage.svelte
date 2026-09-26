<script lang="ts">
  import commonStyles from '../styles/common.module.css';

  interface Props {
    loading: boolean;
    error: string | null;
    empty: boolean;
    loadingText?: string;
    emptyText?: string;
  }

  let {
    loading,
    error,
    empty,
    loadingText = 'Loading...',
    emptyText = 'Nothing here yet'
  }: Props = $props();
</script>

{#if loading}
  <div class="status-message" data-testid="status-loading">
    <span class={commonStyles['text-muted']}>{loadingText}</span>
  </div>
{:else if error}
  <div class="status-message status-error" data-testid="status-error">
    <span class={commonStyles['text-error']}>{error}</span>
  </div>
{:else if empty}
  <div class="status-message" data-testid="status-empty">
    <span class={commonStyles['text-muted']}>{emptyText}</span>
  </div>
{/if}

<style>
  .status-message {
    padding: var(--spacing-lg);
    text-align: center;
  }

  .status-error {
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--color-error-bg);
    border-bottom: 1px solid var(--color-error);
    font-size: var(--font-size-sm);
  }
</style>
