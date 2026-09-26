# Phase 5: Chat Detail — Conversation View, Request Timeline, Raw Logging

## Overview

Fill the main pane (mounted in Phase 4's `viewer-content` section) with the drill-down
experience:

- **`ChatDetailView.svelte`** — main pane for the selected chat: header (title/model/timestamps),
  the reconstructed conversation, and the chronological request timeline.
- **`ConversationView.svelte`** — renders the `conversation` turns delivered by
  `GET /api/chats/:id` (Phase 2's `buildConversation`): role-labeled message turns from the
  latest request's history, then one assistant turn (markdown-rendered when parseable, with
  pending / error states).
- **`RequestTimeline.svelte`** — one row per logged request with ts, model, stream badge, status
  (200 / in-flight / error), and duration; clicking selects a request.
- **`RequestDetailView.svelte`** — metadata plus tabbed raw views: assembled assistant message,
  raw request body (pre-`extraBody`-injection), raw response body (verbatim SSE for streams).
- **`RawBodyView.svelte`** — large-body-safe raw text display with JSON pretty-print fallback.

Together these complete the milestone's core requirement: "review chats content, raw logging".

**Data reminder (Phase 2/3 contracts):** `store.chatDetail: ChatDetail | null` where
`ChatDetail = { chat: ChatSummary, requests: RequestSummary[], conversation: ConversationTurn[] }`;
`store.requestDetail: RequestDetail | null` where
`RequestDetail = { summary: RequestSummary, requestBody: string, responseBody: string | null,
responseAssembled: string | null }`. `ConversationTurn` is a discriminated union on `kind`:
`'message'` (`role`, `content: unknown`) or `'assistant'` (`requestId`, `content: string | null`,
`error: string | null`, `pending: boolean`). A request is in flight iff `status === null &&
error === null`.

## Files to Create

### 1. `frontend-proxy/src/lib/components/RawBodyView.svelte`

```svelte
<script lang="ts">
  interface Props {
    body: string | null;
    /** Optional heading shown above the body (e.g. "Response Body"). */
    label?: string;
  }

  let { body, label = '' }: Props = $props();

  /** Pretty-print JSON bodies; leave everything else (SSE text) verbatim. */
  function displayText(value: string): string {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
</script>

<div class="raw-body">
  {#if label}
    <div class="raw-body-label text-muted">{label}</div>
  {/if}
  {#if body === null}
    <div class="raw-body-empty text-muted">— nothing recorded —</div>
  {:else}
    <pre class="raw-body-pre">{displayText(body)}</pre>
  {/if}
</div>

<style>
  .raw-body {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    min-width: 0;
  }

  .raw-body-label {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .raw-body-empty {
    padding: var(--spacing-sm);
    font-style: italic;
  }

  .raw-body-pre {
    margin: 0;
    padding: var(--spacing-md);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: var(--font-size-xs);
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 480px;
    overflow: auto;
  }
</style>
```

Deliberately low-tech: no syntax highlighting pass over potentially multi-megabyte SSE
concatenations (grand plan AD-5 size note).

### 2. `frontend-proxy/src/lib/components/ConversationView.svelte`

Props-driven (testable without the store). Assistant markdown uses `marked` exactly like
`frontend/src/lib/components/Message.svelte` (`marked.parse(content, { breaks: true })`).

```svelte
<script lang="ts">
  import { marked } from 'marked';
  import type { ConversationTurn } from '../../lib/api/schemas.js';

  interface Props {
    conversation: ConversationTurn[];
  }

  let { conversation }: Props = $props();

  /**
   * Render a message turn's content: OpenAI content is either a string or an
   * array of parts (text / image_url / ...). Strings render as plain text;
   * anything else renders as pretty JSON so nothing is silently dropped.
   */
  function renderMessageContent(content: unknown): string {
    if (typeof content === 'string') {
      return content;
    }
    if (content === null || content === undefined) {
      return '—';
    }
    return JSON.stringify(content, null, 2);
  }

  /**
   * responseAssembled is the assistant message stored as JSON text. Try to
   * extract its `content` string for markdown rendering; fall back to the raw
   * JSON text so tool-call replies stay visible.
   */
  function renderAssistantContent(assembled: string | null): string {
    if (assembled === null) {
      return '';
    }
    try {
      const parsed: unknown = JSON.parse(assembled);
      if (
        parsed !== null && typeof parsed === 'object' &&
        'content' in parsed && typeof (parsed as { content: unknown }).content === 'string'
      ) {
        return marked.parse((parsed as { content: string }).content, { breaks: true }) as string;
      }
    } catch {
      // fall through
    }
    return `<pre>${escapeHtml(assembled)}</pre>`;
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;');
  }
</script>

<div class="conversation">
  {#each conversation as turn, index (index)}
    {#if turn.kind === 'message'}
      <div class="turn turn-{turn.role}" data-testid="turn-{turn.role}">
        <div class="turn-role text-muted">{turn.role}</div>
        <div class="turn-body">{renderMessageContent(turn.content)}</div>
      </div>
    {:else if turn.pending}
      <div class="turn turn-assistant turn-pending" data-testid="turn-assistant-pending">
        <div class="turn-role text-muted">assistant</div>
        <div class="turn-body text-muted">⏳ response in flight (request {turn.requestId})…</div>
      </div>
    {:else if turn.error}
      <div class="turn turn-assistant turn-error" data-testid="turn-assistant-error">
        <div class="turn-role text-error">assistant</div>
        <div class="turn-body text-error">✗ {turn.error}</div>
      </div>
    {:else}
      <div class="turn turn-assistant" data-testid="turn-assistant">
        <div class="turn-role text-muted">assistant</div>
        {@html renderAssistantContent(turn.content)}
      </div>
    {/if}
  {/each}
</div>

<style>
  .conversation {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .turn {
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg);
  }

  .turn-assistant {
    background: var(--color-bg-secondary);
  }

  .turn-pending {
    border-style: dashed;
  }

  .turn-error {
    border-color: var(--color-error);
    background: var(--color-error-bg);
  }

  .turn-role {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: var(--spacing-xs);
  }

  .turn-body {
    white-space: pre-wrap;
    word-break: break-word;
    font-size: var(--font-size-sm);
  }

  /* Markdown emitted by marked for assistant turns. */
  .turn :global(pre) {
    background: var(--color-bg-tertiary);
    padding: var(--spacing-sm);
    border-radius: var(--radius-sm);
    overflow-x: auto;
  }
</style>
```

**Security note:** `renderAssistantContent` may emit `@html`. `marked` output from arbitrary
logged content must be treated as untrusted. Either configure a sanitizer step (e.g.
`sanitize-html`, added as a dependency) or — acceptable for this local debug tool — keep the
`escapeHtml` fallback path and document the trade-off. Pick sanitizing only if trivial to add;
otherwise document in the README (Phase 6) that the viewer trusts its own logging DB.

### 3. `frontend-proxy/src/lib/components/RequestTimeline.svelte`

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { RequestSummary } from '../../lib/api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    requests: RequestSummary[];
    selectedRequestId?: number | null;
  }

  let { requests, selectedRequestId = null }: Props = $props();

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
  <div class="timeline-header text-muted">Requests ({requests.length})</div>
  <ul class="{commonStyles['list']} timeline-items">
    {#each requests as request (request.id)}
      {@const status = statusView(request)}
      <li
        class="{commonStyles['list-item']} timeline-row {selectedRequestId === request.id ? commonStyles['active'] : ''}"
        onclick={() => handleRowClick(request.id)}
        onkeydown={(e) => e.key === 'Enter' && handleRowClick(request.id)}
        role="button"
        tabindex="0"
        aria-selected={selectedRequestId === request.id}
        title={status.title}
      >
        <span class="timeline-time text-muted">{formatClock(request.ts)}</span>
        <span class="timeline-model truncate">{request.model ?? '—'}</span>
        {#if request.stream}
          <span class="{commonStyles['tag']} timeline-badge">SSE</span>
        {/if}
        <span class="timeline-status {status.className}">{status.text}</span>
        <span class="timeline-duration text-muted">
          {request.durationMs === null ? '—' : `${request.durationMs}ms`}
        </span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--color-border);
  }

  .timeline-header {
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
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
```

### 4. `frontend-proxy/src/lib/components/RequestDetailView.svelte`

Props-driven; App-agnostic so tests need no context. Tabs are local UI state (`$state`).

```svelte
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

  <div class="request-detail-meta text-muted">
    {requestDetail.summary.ts} · {requestDetail.summary.method} {requestDetail.summary.path}
    {#if requestDetail.summary.model} · {requestDetail.summary.model}{/if}
    · {requestDetail.summary.status ?? 'in flight'}
    {#if requestDetail.summary.durationMs !== null} · {requestDetail.summary.durationMs}ms{/if}
    {#if requestDetail.summary.error}
      · <span class="text-error">{requestDetail.summary.error}</span>
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
```

### 5. `frontend-proxy/src/lib/components/ChatDetailView.svelte` — store-bound composition

```svelte
<script lang="ts">
  import { flowResult } from 'mobx';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import StatusMessage from './StatusMessage.svelte';
  import ConversationView from './ConversationView.svelte';
  import RequestTimeline from './RequestTimeline.svelte';
  import RequestDetailView from './RequestDetailView.svelte';

  const store = getProxyLogsStore();

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
      <h2 class="chat-detail-title truncate">{detail.chat.title}</h2>
      <div class="chat-detail-meta text-muted">
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
```

### 6. Wire into `frontend-proxy/src/App.svelte`

In Phase 4's `App.svelte`, replace the `{:else}` placeholder branch of `viewer-content`:

```svelte
{:else}
  <ChatDetailView />
{/if}
```

and add `import ChatDetailView from './lib/components/ChatDetailView.svelte';` to the script
(remove the Phase 5 placeholder comment). The `handleChatSelect` flow is unchanged —
`openChat` populates `store.chatDetail`, which this view renders.

### 7. Test harnesses

**`frontend-proxy/src/lib/components/ChatDetailViewHarness.svelte`** (context-provided mock
store):

```svelte
<script lang="ts">
  import ChatDetailView from './ChatDetailView.svelte';
  import { setProxyLogsStore } from '../../context.js';
  import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';

  interface Props {
    store: ProxyLogsStore;
  }

  let { store }: Props = $props();

  (() => {
    setProxyLogsStore(store);
  })();
</script>

<ChatDetailView />
```

(`ConversationView`, `RequestTimeline`, `RequestDetailView`, `RawBodyView` take plain props —
render them directly, no harness needed.)

### 8. Tests

**`RawBodyView.test.ts`:**

1. Pretty-prints valid JSON (`{"a":1}` renders multi-line indented).
2. Renders non-JSON (SSE `data: ...\n\ndata: [DONE]`) verbatim.
3. `body: null` renders `— nothing recorded —`.

**`ConversationView.test.ts`** (fixture: build `ConversationTurn[]` directly):

1. Renders message turns with role labels (`turn-system`, `turn-user` testids) and string
   contents.
2. Non-string content (array of parts) renders as JSON text, no crash.
3. Completed assistant turn: markdown output present (e.g. content `**bold**` renders a
   `<strong>` or literal `**bold**` depending on marked config — assert on the parsed-content
   path via `data-testid="turn-assistant"` existence and non-empty body).
4. Assistant turn with `content: '{"content":"hello","tool_calls":[...]}'` where content is a
   string → markdown path; assembled JSON **without** string `content` (tool-call only) → escaped
   `<pre>` fallback containing the raw JSON.
5. `pending: true` turn renders `turn-assistant-pending` with the request id.
6. `error` turn renders `turn-assistant-error` with the error text.

**`RequestTimeline.test.ts`:**

1. Renders one row per request with model and count header `Requests (2)`.
2. Completed 200 → `200` with success styling; `status: null, error: null` → `⏳` pending; error
   row → `ERR` and row `title` contains the error text.
3. `stream: true` renders the `SSE` badge; false does not.
4. `durationMs: 1234` renders `1234ms`; null renders `—`.
5. Clicking a row dispatches `requestSelect` with the request id; selected row has
   `aria-selected="true"`.

**`RequestDetailView.test.ts`** (fixture: a full `RequestDetail`):

1. Renders metadata (id, method+path, status, duration).
2. Default tab is Raw Request (pretty-printed request body visible).
3. Clicking `Raw Response` shows the response body; clicking `Assembled Reply` shows assembled.
4. `responseBody: null` (in flight) shows `— nothing recorded —`.
5. Close button calls the `onClose` spy.

**`ChatDetailView.test.ts`** (mock store object with getters, pattern from Phase 4):

1. `detailLoading: true` → "Loading chat..." message.
2. `detailError` set → error rendered.
3. Happy path: chat title, model, and conversation turns rendered; `requests.length` visible in
   timeline header.
4. With `requestDetail` set on the store → `request-detail` testid present; clicking close
   (`request-detail-close`) calls the mock's `closeRequest`.
5. Clicking a timeline row calls the mock's `openRequest` with the row's request id.

## Files to Modify

### `frontend-proxy/src/App.svelte`

As described in step 6: import `ChatDetailView`, replace the placeholder branch.

## Implementation Notes

1. **Conversation rule (grand plan AD-6, refined):** the server already deduplicates — Phase 2's
   `buildConversation` returns the latest request's history as message turns plus **one**
   assistant turn. This component must NOT re-fetch or re-derive; it only renders
   `detail.conversation`. Append-only harnesses embed earlier assistant turns in the history, so
   interleaving every request's assembled reply would duplicate them.
2. **Why the drill-down lives below the timeline (not a modal):** raw bodies are tall; an inline
   expanding section keeps scroll context and avoids modal sizing problems with megabyte SSE
   bodies. `closeRequest()` in the store keeps this a one-liner.
3. **`{@html}` trust boundary:** assistant markdown comes from the logging DB (what the upstream
   model produced). For this local-only tool the risk is accepted; if sanitizing is added later,
   it belongs in `renderAssistantContent`. Document in Phase 6's README.
4. **Turn keys:** conversation turns carry no stable id — key the `{#each}` by index (turns are
   replaced wholesale on refresh, never spliced).
5. **In-flight vs errored:** check `error` **before** `status === null` (an errored exchange also
   has `status NULL`); `statusView` in RequestTimeline and the `{:else if}` chain in
   ConversationView both encode this order — keep it consistent if refactoring.
6. **Large bodies:** `RawBodyView` caps height (`max-height: 480px`, `overflow: auto`) so one
   huge SSE dump cannot blow up the layout; `white-space: pre-wrap` keeps long lines from
   forcing horizontal scroll.
7. **All five components stay well under the 500-line file limit.**

## Dependencies

- **Depends on:** Phase 4 (`App.svelte` shell, `StatusMessage`, component conventions) and
  Phase 3 (store contract: `chatDetail`, `requestDetail`, `openRequest`, `closeRequest`). Leaf
  components (`RawBodyView`, `ConversationView`, `RequestTimeline`, `RequestDetailView`) depend
  only on Phase 2's schema types and can be built in parallel with Phase 4.
- **Blocks:** Phase 6 documents the finished UI.
- **This is the last code phase** — completing it fulfills the milestone's feature scope.

## Verification Summary

```sh
npm --prefix frontend-proxy run test     # all new component tests green
npm --prefix frontend-proxy run check    # svelte-check green
# manual end-to-end:
# 1. terminal A: cargo run -p rhd_ai_proxy -- --config packages/rhd_ai_proxy/proxy.example.yaml
# 2. terminal B: VITE_PROXY_LOGS_PATH=packages/rhd_ai_proxy/proxy-logs npm --prefix frontend-proxy run dev
# 3. curl -N localhost:1234/v1/chat/completions -H 'content-type: application/json' \
#      -d '{"model":"z-ai/glm-5.3","messages":[{"role":"user","content":"hi"}],"stream":true}'
# 4. open http://localhost:5174 → chat appears; click it → conversation + timeline;
#    click the request row → raw request/response/assembled tabs; press Refresh mid-stream →
#    pending turn becomes the completed reply.
```
