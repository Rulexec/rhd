<script lang="ts">
  import { flowResult } from 'mobx';
  import { tick } from 'svelte';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import StatusMessage from './StatusMessage.svelte';
  import ConversationView from './ConversationView.svelte';
  import ConversationIndex from './ConversationIndex.svelte';
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
  const openRequestAnchorGetter = mobxObservable(() => store.openRequestAnchor);

  let detail = $derived(detailGetter());
  let detailLoading = $derived(detailLoadingGetter());
  let detailError = $derived(detailErrorGetter());
  let requestDetail = $derived(requestDetailGetter());
  let selectedRequestId = $derived(selectedRequestIdGetter());
  let openRequestAnchor = $derived(openRequestAnchorGetter());

  /**
   * Whether the open drill-down still anchors inside the conversation (its
   * turn exists / the tail is still pending or errored). False → render at
   * the bottom section below the timeline; this is also the refresh
   * fallback when the anchored turn no longer exists.
   */
  let anchoredInConversation = $derived.by(() => {
    if (!detail || openRequestAnchor === null) {
      return false;
    }
    if (openRequestAnchor === 'tail') {
      const last = detail.conversation.at(-1);
      return last !== undefined && (last.kind === 'pending' || last.kind === 'error');
    }
    return detail.conversation.some(
      (turn) => turn.kind === 'message' && turn.seq === openRequestAnchor
    );
  });

  // The scrolling column — turn/section scroll targets are queried inside it.
  let scrollContainer: HTMLDivElement | null = $state(null);

  /**
   * Open a request and scroll its drill-down into view once rendered. The
   * `[data-testid="request-detail"]` selector matches wherever the drill-down
   * landed (inline after the anchored turn or the bottom section).
   */
  async function openAndScroll(requestId: number, anchor: number | 'tail' | null): Promise<void> {
    try {
      await flowResult(store.openRequest(requestId, anchor));
    } catch {
      // The store surfaces load errors in the detail area.
    }
    await tick();
    const detailEl = scrollContainer?.querySelector('[data-testid="request-detail"]');
    if (detailEl) {
      scrollIntoViewSafe(detailEl);
    }
  }

  /**
   * Turn `raw #N` buttons toggle: clicking the request that is already open
   * at that same anchor closes it; any other click opens (and re-anchors)
   * the drill-down inline after the clicked turn. Toggle identity is the
   * turn (anchor), not the request — sibling turns sharing a requestId
   * re-anchor instead of closing.
   */
  function handleTurnRequestSelect(event: CustomEvent<{ requestId: number; anchor: number | 'tail' }>) {
    const { requestId, anchor } = event.detail;
    if (requestDetail?.summary.id === requestId && openRequestAnchor === anchor) {
      store.closeRequest();
      return;
    }
    void openAndScroll(requestId, anchor);
  }

  /** Timeline rows keep the bottom placement (the user is already there). */
  function handleTimelineRequestSelect(event: CustomEvent<{ requestId: number }>) {
    void openAndScroll(event.detail.requestId, null);
  }

  function handleCloseRequest() {
    store.closeRequest();
  }

  /** Index entry click: scroll the conversation to the turn and flash it. */
  function handleIndexEntrySelect(event: CustomEvent<{ seq: number }>) {
    const turnEl = scrollContainer?.querySelector(`[data-turn-seq="${event.detail.seq}"]`);
    if (!turnEl) {
      return;
    }
    scrollIntoViewSafe(turnEl);
    turnEl.classList.add('turn-flash');
    window.setTimeout(() => turnEl.classList.remove('turn-flash'), 1200);
  }

  /** jsdom implements neither smooth scrolling nor scrollIntoView — guard both. */
  function scrollIntoViewSafe(el: Element): void {
    if (typeof el.scrollIntoView === 'function') {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
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

    <div class="chat-detail-body">
      <div class="chat-detail-scroll" bind:this={scrollContainer}>
        <section class="chat-detail-section">
          <ConversationView
            conversation={detail.conversation}
            requestDetail={anchoredInConversation ? requestDetail : null}
            openAnchor={openRequestAnchor}
            onCloseRequest={handleCloseRequest}
            on:requestSelect={handleTurnRequestSelect}
          />
        </section>

        <section class="chat-detail-section">
          <RequestTimeline
            requests={detail.requests}
            {selectedRequestId}
            on:requestSelect={handleTimelineRequestSelect}
          />
        </section>

        {#if requestDetail && !anchoredInConversation}
          <section class="chat-detail-section">
            <RequestDetailView {requestDetail} onClose={handleCloseRequest} />
          </section>
        {/if}
      </div>

      <aside class="chat-detail-index">
        <ConversationIndex
          conversation={detail.conversation}
          on:entrySelect={handleIndexEntrySelect}
        />
      </aside>
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

  .chat-detail-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .chat-detail-scroll {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
  }

  .chat-detail-index {
    width: 280px;
    flex-shrink: 0;
    border-left: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    overflow: hidden;
  }

  .chat-detail-section {
    min-width: 0;
  }
</style>
