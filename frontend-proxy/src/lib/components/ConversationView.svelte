<script lang="ts">
  import { marked } from 'marked';
  import type { ConversationTurn } from '../../lib/api/schemas.js';
  import commonStyles from '../styles/common.module.css';

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
        <div class="turn-role {commonStyles['text-muted']}">{turn.role}</div>
        <div class="turn-body">{renderMessageContent(turn.content)}</div>
      </div>
    {:else if turn.pending}
      <div class="turn turn-assistant turn-pending" data-testid="turn-assistant-pending">
        <div class="turn-role {commonStyles['text-muted']}">assistant</div>
        <div class="turn-body {commonStyles['text-muted']}">⏳ response in flight (request {turn.requestId})…</div>
      </div>
    {:else if turn.error}
      <div class="turn turn-assistant turn-error" data-testid="turn-assistant-error">
        <div class="turn-role {commonStyles['text-error']}">assistant</div>
        <div class="turn-body {commonStyles['text-error']}">✗ {turn.error}</div>
      </div>
    {:else}
      <div class="turn turn-assistant" data-testid="turn-assistant">
        <div class="turn-role {commonStyles['text-muted']}">assistant</div>
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
