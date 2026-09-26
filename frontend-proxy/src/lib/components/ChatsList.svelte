<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import StatusMessage from './StatusMessage.svelte';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    selectedChatId?: number | null;
  }

  let { selectedChatId = null }: Props = $props();

  const dispatch = createEventDispatcher<{
    chatSelect: { chatId: number };
  }>();

  const store = getProxyLogsStore();

  // Bridge MobX observables to Svelte reactivity (top-level calls required).
  const chatsGetter = mobxObservable(() => store.chats);
  const hasChatsGetter = mobxObservable(() => store.hasChats);
  const loadingGetter = mobxObservable(() => store.chatsLoading);
  const errorGetter = mobxObservable(() => store.chatsError);

  let chats = $derived(chatsGetter());
  let hasChats = $derived(hasChatsGetter());
  let loading = $derived(loadingGetter());
  let error = $derived(errorGetter());

  function handleChatClick(chatId: number): void {
    dispatch('chatSelect', { chatId });
  }

  function formatTime(dateString: string): string {
    const date = new Date(dateString);
    const diffMs = Date.now() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);
    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="chats-list">
  <div class="chats-list-header">
    <span class={commonStyles['text-muted']}>Chats</span>
  </div>

  <StatusMessage
    {loading}
    {error}
    empty={!hasChats}
    loadingText="Loading chats..."
    emptyText="No logged chats yet — send traffic through rhd_ai_proxy and press Refresh"
  />

  {#if !loading && !error && hasChats}
    <ul class="{commonStyles['list']} chats-list-items" role="listbox" aria-label="Chats">
      {#each chats as chat (chat.id)}
        <li
          class="{commonStyles['list-item']} {selectedChatId === chat.id ? commonStyles['active'] : ''}"
          onclick={() => handleChatClick(chat.id)}
          onkeydown={(e) => e.key === 'Enter' && handleChatClick(chat.id)}
          role="option"
          tabindex="0"
          aria-selected={selectedChatId === chat.id}
        >
          <div class="chat-item-content">
            <div class="chat-item-title {commonStyles['truncate']}">{chat.title}</div>
            <div class="chat-item-meta">
              {#if chat.model}
                <span class="chat-item-model {commonStyles['truncate']}">{chat.model}</span>
              {/if}
              <span class="chat-item-count {commonStyles['text-muted']}">{chat.requestCount} req</span>
              <span class="chat-item-time {commonStyles['text-muted']}">{formatTime(chat.updatedAt)}</span>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .chats-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chats-list-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    font-size: var(--font-size-sm);
    font-weight: 500;
  }

  .chats-list-items {
    flex: 1;
    overflow-y: auto;
  }

  .chat-item-content {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .chat-item-title {
    font-weight: 500;
    font-size: var(--font-size-sm);
  }

  .chat-item-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    font-size: var(--font-size-xs);
  }

  .chat-item-model {
    max-width: 140px;
    color: var(--color-text-secondary);
  }

  .chat-item-count,
  .chat-item-time {
    white-space: nowrap;
  }
</style>
