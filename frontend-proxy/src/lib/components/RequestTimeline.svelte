<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { RequestSummary } from '../../lib/api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    requests: RequestSummary[];
    selectedRequestId?: number | null;
  }

  let { requests, selectedRequestId = null }: Props = $props();

  // UI-local collapse state: the raw-requests list is long, so it starts
  // collapsed; the user expands it via the header when interested.
  let collapsed = $state(true);

  const dispatch = createEventDispatcher<{
    requestSelect: { requestId: number };
  }>();

  function handleRowClick(requestId: number): void {
    dispatch('requestSelect', { requestId });
  }

  function formatClock(ts: string): string {
    return new Date(ts).toLocaleTimeString();
  }

  type StatusView = { text: string; className: string; title: string };

  // Check error before status === null: an errored exchange also has status NULL.
  function statusView(request: RequestSummary): StatusView {
    if (request.error) {
      return { text: 'ERR', className: 'st-error', title: request.error };
    }
    if (request.status === null) {
      return { text: '⏳', className: 'st-pending', title: 'response in flight' };
    }
    const ok = request.status >= 200 && request.status < 300;
    return {
      text: String(request.status),
      className: ok ? 'st-ok' : 'st-error',
      title: `HTTP ${request.status}`
    };
  }
</script>

<div class="timeline">
  <button
    class="timeline-header"
    onclick={() => (collapsed = !collapsed)}
    aria-expanded={!collapsed}
    data-testid="requests-toggle"
  >
    <span class="timeline-header-text {commonStyles['text-muted']}">
      {collapsed ? '▸' : '▾'} Requests ({requests.length})
      {#if collapsed && selectedRequestId !== null}
        <span class="timeline-selected-hint {commonStyles['text-muted']}">
          · #{selectedRequestId} selected
        </span>
      {/if}
    </span>
  </button>
  {#if !collapsed}
    <ul class="{commonStyles['list']} timeline-items" role="listbox" aria-label="Requests">
      {#each requests as request (request.id)}
        {@const status = statusView(request)}
        {@const selected = selectedRequestId === request.id}
        <li
          class="{commonStyles['list-item']} timeline-row {selected ? commonStyles['active'] : ''}"
          onclick={() => handleRowClick(request.id)}
          onkeydown={(e) => e.key === 'Enter' && handleRowClick(request.id)}
          role="option"
          tabindex="0"
          aria-selected={selected}
          title={status.title}
        >
          <span class="timeline-time {commonStyles['text-muted']}">{formatClock(request.ts)}</span>
          <span class="timeline-model {commonStyles['truncate']}">{request.model ?? '—'}</span>
          {#if request.stream}
            <span class="{commonStyles['tag']} timeline-badge">SSE</span>
          {/if}
          <span class="timeline-status {status.className}">{status.text}</span>
          <span class="timeline-duration {commonStyles['text-muted']}">
            {request.durationMs === null ? '—' : `${request.durationMs}ms`}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--color-border);
  }

  .timeline-header {
    display: flex;
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    background: none;
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    text-align: left;
    cursor: pointer;
  }

  .timeline-header-text {
    font-size: var(--font-size-xs);
  }

  .timeline-selected-hint {
    text-transform: none;
    letter-spacing: normal;
  }

  .timeline-items {
    flex: 1;
    overflow-y: auto;
  }

  .timeline-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    font-size: var(--font-size-xs);
  }

  .timeline-time {
    white-space: nowrap;
  }

  .timeline-model {
    flex: 1;
    min-width: 0;
    color: var(--color-text-secondary);
  }

  .timeline-badge {
    font-size: var(--font-size-xs);
  }

  .timeline-status {
    font-weight: 600;
    white-space: nowrap;
  }

  .st-ok {
    color: var(--color-success);
  }

  .st-error {
    color: var(--color-error);
  }

  .st-pending {
    color: var(--color-warning);
  }

  .timeline-duration {
    white-space: nowrap;
    min-width: 52px;
    text-align: right;
  }
</style>
