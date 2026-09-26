<script lang="ts">
  import { flowResult } from 'mobx';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import StatusMessage from './StatusMessage.svelte';
  import ConversationView from './ConversationView.svelte';
  import RequestTimeline from './RequestTimeline.svelte';
  import RequestDetailView from './RequestDetailView.svelte';
  import commonStyles from '../styles/common.module.css';

  const store = getProxyLogsStore();

  // Bridge MobX observables to Svelte reactivity (top-level calls required).
  const detailGetter = mobxObservable(() => store.chatDetail);
  const detailLoadingGetter = mobxObservable(() => store.detailLoading);
  const detailErrorGetter = mobxObservable(() => store.detailError);
  const requestDetailGetter = mobxObservable(() => store.requestDetail);
  const selectedRequestIdGetter = mobxObservable(() => store.selectedRequestId);

  let detail = $derived(detailGetter());
  let detailLoading = $derived(detailLoadingGetter());
  let detailError = $derived(detailErrorGetter());
  let requestDetail = $derived(requestDetailGetter());
  let selectedRequestId = $derived(selectedRequestIdGetter());

  function handleRequestSelect(event: CustomEvent<{ requestId: number }>) {
    void flowResult(store.openRequest(event.detail.requestId)).catch(() => {});
  }

  function handleCloseRequest() {
    store.closeRequest();
  }
</script>

<div class="chat-detail">
  {#if detailLoading}
    <StatusMessage loading={true} error={null} empty={false} loadingText="Loading chat..." />
  {:else if detailError}
    <StatusMessage loading={false} error={detailError} empty={false} />
  {:else if detail}
    <header class="chat-detail-header">
      <h2 class="chat-detail-title {commonStyles['truncate']}">{detail.chat.title}</h2>
      <div class="chat-detail-meta {commonStyles['text-muted']}">
        {detail.chat.model ?? 'unknown model'} · created {new Date(detail.chat.createdAt).toLocaleString()}
        · updated {new Date(detail.chat.updatedAt).toLocaleString()}
      </div>
    </header>

    <div class="chat-detail-scroll">
      <section class="chat-detail-section">
        <ConversationView conversation={detail.conversation} />
      </section>

      <section class="chat-detail-section">
        <RequestTimeline
          requests={detail.requests}
          {selectedRequestId}
          on:requestSelect={handleRequestSelect}
        />
      </section>

      {#if requestDetail}
        <section class="chat-detail-section">
          <RequestDetailView {requestDetail} onClose={handleCloseRequest} />
        </section>
      {/if}
    </div>
  {:else}
    <StatusMessage loading={false} error={null} empty={true} emptyText="No chat selected" />
  {/if}
</div>

<style>
  .chat-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .chat-detail-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
  }

  .chat-detail-title {
    margin: 0 0 var(--spacing-xs) 0;
    font-size: var(--font-size-lg);
  }

  .chat-detail-meta {
    font-size: var(--font-size-xs);
  }

  .chat-detail-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
  }

  .chat-detail-section {
    min-width: 0;
  }
</style>
