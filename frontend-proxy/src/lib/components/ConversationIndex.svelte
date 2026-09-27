<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ConversationTurn, MessageTurn } from '../api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    conversation: ConversationTurn[];
  }

  let { conversation }: Props = $props();

  const dispatch = createEventDispatcher<{
    entrySelect: { seq: number };
  }>();

  /**
   * Index entries: user messages and assistant responses that made no tool
   * calls — the navigable skeleton of the conversation. System prompts, tool
   * traffic, and tool-calling assistant turns are excluded.
   */
  let entries = $derived(
    conversation.filter((turn): turn is MessageTurn & { kind: 'message' } => {
      if (turn.kind !== 'message') {
        return false;
      }
      if (turn.role === 'user') {
        return true;
      }
      return turn.role === 'assistant' && (turn.toolCalls === null || turn.toolCalls.length === 0);
    })
  );

  function handleEntryClick(seq: number): void {
    dispatch('entrySelect', { seq });
  }

  /**
   * One-line preview text of an entry: string content as-is; arrays of parts
   * (text / image_url / ...) contribute their text fields; anything else
   * falls back to compact JSON. Whitespace collapses to single spaces (the
   * preview is one line); never throws on odd shapes.
   */
  function previewText(turn: MessageTurn): string {
    let text: string;
    if (typeof turn.content === 'string') {
      text = turn.content;
    } else if (Array.isArray(turn.content)) {
      text = turn.content
        .map((part) =>
          part !== null && typeof part === 'object' && 'text' in part
            ? String((part as { text: unknown }).text ?? '')
            : ''
        )
        .join(' ');
    } else if (turn.content === null || turn.content === undefined) {
      text = '';
    } else {
      try {
        text = JSON.stringify(turn.content) ?? '';
      } catch {
        text = '';
      }
    }
    const normalized = text.replace(/\s+/g, ' ').trim();
    return normalized === '' ? '—' : normalized;
  }
</script>

<div class="conversation-index" data-testid="conversation-index">
  <div class="conversation-index-header {commonStyles['text-muted']}">
    Index ({entries.length})
  </div>
  {#if entries.length === 0}
    <div class="conversation-index-empty {commonStyles['text-muted']}">
      No user or assistant messages
    </div>
  {:else}
    <ul class="{commonStyles['list']} conversation-index-items" role="listbox" aria-label="Conversation index">
      {#each entries as entry (entry.seq)}
        <li
          class="{commonStyles['list-item']} conversation-index-entry"
          data-testid="index-entry"
          onclick={() => handleEntryClick(entry.seq)}
          onkeydown={(e) => e.key === 'Enter' && handleEntryClick(entry.seq)}
          role="option"
          tabindex="0"
          aria-selected="false"
          title={previewText(entry)}
        >
          <span class="index-entry-role role-{entry.role}">{entry.role}</span>
          <span class="index-entry-preview {commonStyles['truncate']}">{previewText(entry)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .conversation-index {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .conversation-index-header {
    padding: var(--spacing-sm) var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .conversation-index-empty {
    padding: var(--spacing-md);
    font-size: var(--font-size-xs);
    font-style: italic;
  }

  .conversation-index-items {
    flex: 1;
    overflow-y: auto;
  }

  .conversation-index-entry {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: var(--font-size-xs);
    cursor: pointer;
  }

  .index-entry-role {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0 var(--spacing-xs);
  }

  .role-user {
    color: var(--color-primary);
  }

  .role-assistant {
    color: var(--color-text-secondary);
  }

  .index-entry-preview {
    flex: 1;
    min-width: 0;
    color: var(--color-text-secondary);
  }
</style>
