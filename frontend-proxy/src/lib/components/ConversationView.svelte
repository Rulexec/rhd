<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { marked } from 'marked';
  import type { ConversationTurn, MessageTurn } from '../api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    conversation: ConversationTurn[];
  }

  let { conversation }: Props = $props();

  const dispatch = createEventDispatcher<{
    requestSelect: { requestId: number };
  }>();

  function handleRequestClick(requestId: number): void {
    dispatch('requestSelect', { requestId });
  }

  /** Display metadata of one tool_calls entry (navigated defensively). */
  function toolCallMeta(call: unknown): {
    name: string | null;
    id: string | null;
    arguments: string | null;
  } {
    if (call === null || typeof call !== 'object') {
      return { name: null, id: null, arguments: null };
    }
    const record = call as { id?: unknown; function?: { name?: unknown; arguments?: unknown } };
    const fn = record.function ?? {};
    const args =
      typeof fn.arguments === 'string'
        ? prettyJson(fn.arguments)
        : fn.arguments === undefined || fn.arguments === null
          ? null
          : JSON.stringify(fn.arguments, null, 2);
    return {
      name: typeof fn.name === 'string' ? fn.name : null,
      id: typeof record.id === 'string' ? record.id : null,
      arguments: args
    };
  }

  /** Pretty-print a string that itself contains JSON; fall back to it verbatim. */
  function prettyJson(text: string): string {
    try {
      return JSON.stringify(JSON.parse(text) as unknown, null, 2);
    } catch {
      return text;
    }
  }

  /**
   * Plain-text rendering of a message turn's content: OpenAI content is either
   * a string or an array of parts (text / image_url / ...). Strings render as
   * text (tool results pretty-print their JSON payloads); anything else renders
   * as pretty JSON so nothing is silently dropped. Assistant strings render as
   * markdown via {@html renderAssistantMarkdown} in the template instead.
   */
  function renderMessageContent(turn: MessageTurn): string {
    if (typeof turn.content === 'string') {
      return turn.role === 'tool' ? prettyJson(turn.content) : turn.content;
    }
    if (turn.content === null || turn.content === undefined) {
      return '—';
    }
    return JSON.stringify(turn.content, null, 2);
  }

  /** Assistant markdown: every assistant row with string content, not just the last. */
  function renderAssistantMarkdown(content: string): string {
    return marked.parse(content, { breaks: true }) as string;
  }
</script>

<div class="conversation">
  {#each conversation as turn, index (index)}
    {#if turn.kind === 'message'}
      <div class="turn turn-{turn.role}" data-testid="turn-{turn.role}" data-turn-seq={turn.seq}>
        <div class="turn-role {commonStyles['text-muted']}">
          {#if turn.role === 'tool'}
            tool: {turn.name ?? turn.toolCallId ?? 'unknown'}
          {:else}
            {turn.role}
          {/if}
          <button
            class="turn-request-button"
            onclick={() => handleRequestClick(turn.requestId)}
            data-testid="turn-request-button"
            title="Open the raw exchange behind this turn — the request whose payload first carried it (history) or produced it (response)"
          >
            raw #{turn.requestId}
          </button>
        </div>
        {#if turn.role === 'assistant' && typeof turn.content === 'string'}
          <div class="turn-body">{@html renderAssistantMarkdown(turn.content)}</div>
        {:else}
          <div class="turn-body">{renderMessageContent(turn)}</div>
        {/if}
        {#if turn.toolCalls}
          <div class="tool-calls" data-testid="tool-calls">
            {#each turn.toolCalls as call, callIndex (callIndex)}
              {@const meta = toolCallMeta(call)}
              <div class="tool-call" data-testid="tool-call">
                <div class="tool-call-head">
                  <span>🔧 {meta.name ?? 'unknown function'}</span>
                  {#if meta.id}
                    <span class="{commonStyles['text-muted']} tool-call-id">{meta.id}</span>
                  {/if}
                </div>
                {#if meta.arguments !== null}
                  <pre class="tool-call-args">{meta.arguments}</pre>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
        {#if turn.role === 'tool' && turn.toolCallId}
          <div class="tool-call-ref {commonStyles['text-muted']}">tool_call_id: {turn.toolCallId}</div>
        {/if}
      </div>
    {:else if turn.kind === 'pending'}
      <div class="turn turn-assistant turn-pending" data-testid="turn-pending">
        <div class="turn-role {commonStyles['text-muted']}">
          assistant
          <button
            class="turn-request-button"
            onclick={() => handleRequestClick(turn.requestId)}
            data-testid="turn-request-button"
            title="Open the raw exchange behind this turn — the request whose payload first carried it (history) or produced it (response)"
          >
            raw #{turn.requestId}
          </button>
        </div>
        <div class="turn-body {commonStyles['text-muted']}">⏳ response in flight…</div>
      </div>
    {:else if turn.kind === 'error'}
      <div class="turn turn-assistant turn-error" data-testid="turn-error">
        <div class="turn-role {commonStyles['text-error']}">
          assistant
          <button
            class="turn-request-button"
            onclick={() => handleRequestClick(turn.requestId)}
            data-testid="turn-request-button"
            title="Open the raw exchange behind this turn — the request whose payload first carried it (history) or produced it (response)"
          >
            raw #{turn.requestId}
          </button>
        </div>
        <div class="turn-body {commonStyles['text-error']}">✗ {turn.error}</div>
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
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
  }

  .turn-request-button {
    margin-left: auto;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: none;
    padding: 0 var(--spacing-xs);
    font-size: var(--font-size-xs);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
  }

  .turn-request-button:hover {
    border-color: var(--color-primary);
    color: var(--color-text);
  }

  /* Applied imperatively by ChatDetailView when an index entry jumps to a turn. */
  :global(.turn-flash) {
    animation: turn-flash 1.2s ease-out;
  }

  @keyframes turn-flash {
    0% {
      background: var(--color-primary);
    }
    100% {
      background: transparent;
    }
  }

  .turn-body {
    white-space: pre-wrap;
    word-break: break-word;
    font-size: var(--font-size-sm);
  }

  .tool-calls {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    margin-top: var(--spacing-xs);
  }

  .tool-call {
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--spacing-xs) var(--spacing-sm);
  }

  .tool-call-head {
    display: flex;
    justify-content: space-between;
    gap: var(--spacing-sm);
    font-size: var(--font-size-xs);
  }

  .tool-call-args {
    margin: var(--spacing-xs) 0 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: var(--font-size-xs);
  }

  .tool-call-ref {
    font-size: var(--font-size-xs);
    margin-top: var(--spacing-xs);
  }

  /* Markdown emitted by marked for assistant turns. */
  .turn :global(pre) {
    background: var(--color-bg-tertiary);
    padding: var(--spacing-sm);
    border-radius: var(--radius-sm);
    overflow-x: auto;
  }
</style>
