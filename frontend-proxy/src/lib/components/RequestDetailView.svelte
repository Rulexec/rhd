<script lang="ts">
  import type { RequestDetail } from '../../lib/api/schemas.js';
  import RawBodyView from './RawBodyView.svelte';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    requestDetail: RequestDetail;
    onClose: () => void;
  }

  let { requestDetail, onClose }: Props = $props();

  type Tab = 'assembled' | 'request' | 'response';
  let activeTab: Tab = $state('request');

  const tabs: { id: Tab; label: string }[] = [
    { id: 'request', label: 'Raw Request' },
    { id: 'response', label: 'Raw Response' },
    { id: 'assembled', label: 'Assembled Reply' }
  ];
</script>

<div class="request-detail" data-testid="request-detail">
  <div class="request-detail-header">
    <span class="request-detail-title">
      Request #{requestDetail.summary.id}
      {#if requestDetail.summary.stream}<span class="{commonStyles['tag']}">SSE</span>{/if}
    </span>
    <button
      class="{commonStyles['btn']} {commonStyles['btn-ghost']} {commonStyles['btn-sm']}"
      onclick={onClose}
      data-testid="request-detail-close"
    >
      × Close
    </button>
  </div>

  <div class="request-detail-meta {commonStyles['text-muted']}">
    {requestDetail.summary.ts} · {requestDetail.summary.method} {requestDetail.summary.path}
    {#if requestDetail.summary.model} · {requestDetail.summary.model}{/if}
    · {requestDetail.summary.status ?? 'in flight'}
    {#if requestDetail.summary.durationMs !== null} · {requestDetail.summary.durationMs}ms{/if}
    {#if requestDetail.summary.error}
      · <span class="{commonStyles['text-error']}">{requestDetail.summary.error}</span>
    {/if}
  </div>

  <div class="request-detail-tabs" role="tablist">
    {#each tabs as tab (tab.id)}
      <button
        class="tab-button {activeTab === tab.id ? 'tab-active' : ''}"
        role="tab"
        aria-selected={activeTab === tab.id}
        onclick={() => (activeTab = tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="request-detail-body">
    {#if activeTab === 'request'}
      <RawBodyView body={requestDetail.requestBody} label="Request Body — original bytes before extraBody injection" />
    {:else if activeTab === 'response'}
      <RawBodyView body={requestDetail.responseBody} label="Response Body — raw, SSE bytes verbatim for streams" />
    {:else}
      <RawBodyView body={requestDetail.responseAssembled} label="Assembled Assistant Message" />
    {/if}
  </div>
</div>

<style>
  .request-detail {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--spacing-md);
    background: var(--color-bg);
  }

  .request-detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .request-detail-title {
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .request-detail-meta {
    font-size: var(--font-size-xs);
  }

  .request-detail-tabs {
    display: flex;
    gap: var(--spacing-xs);
    border-bottom: 1px solid var(--color-border);
  }

  .tab-button {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: var(--spacing-xs) var(--spacing-sm);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

  .tab-active {
    color: var(--color-text);
    border-bottom-color: var(--color-primary);
  }

  .request-detail-body {
    min-height: 0;
  }
</style>
