<script lang="ts">
  import { flowResult } from 'mobx';
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import ConfirmModal from './ConfirmModal.svelte';
  import commonStyles from '../styles/common.module.css';

  import { createEventDispatcher } from 'svelte';

  interface Props {
    selectedChatId?: number | null;
  }

  let { selectedChatId = null }: Props = $props();

  const dispatch = createEventDispatcher<{
    chatSelect: { chatId: number };
  }>();

  const appStore = getAppStore();
  const chatsListStore = appStore.chatsList;

  // Bridge MobX observables to Svelte reactivity.
  // mobxObservable returns a getter and must be invoked at component top level.
  const chatsGetter = mobxObservable(() => chatsListStore.chats);
  const hasChatsGetter = mobxObservable(() => chatsListStore.hasChats);
  const chatsLoadingGetter = mobxObservable(() => chatsListStore.loading);
  const chatsErrorGetter = mobxObservable(() => chatsListStore.error);
  const isConnectedGetter = mobxObservable(() => appStore.connection.isConnected);

  let chats = $derived(chatsGetter());
  let hasChats = $derived(hasChatsGetter());
  let chatsLoading = $derived(chatsLoadingGetter());
  let chatsError = $derived(chatsErrorGetter());
  let isConnected = $derived(isConnectedGetter());

  let showDeleteAllModal: boolean = $state(false);
  let isCreating: boolean = $state(false);
  let isDeleting: boolean = $state(false);

  async function handleCreateChat() {
    if (!isConnected) {
      return;
    }

    isCreating = true;
    try {
      const chatId = await flowResult(chatsListStore.createNewChat());
      if (chatId !== null) {
        dispatch('chatSelect', { chatId });
      }
    } catch (error) {
      console.error('Failed to create chat:', error);
    } finally {
      isCreating = false;
    }
  }

  function handleDeleteAllClick() {
    showDeleteAllModal = true;
  }

  async function handleDeleteAllConfirm() {
    showDeleteAllModal = false;
    isDeleting = true;
    try {
      const success = await chatsListStore.deleteAllChats();
      if (!success) {
        console.error('Failed to delete all chats');
      }
    } catch (error) {
      console.error('Failed to delete all chats:', error);
    } finally {
      isDeleting = false;
    }
  }

  function handleDeleteAllCancel() {
    showDeleteAllModal = false;
  }

  function handleChatClick(chatId: number) {
    dispatch('chatSelect', { chatId });
  }

  function formatTime(dateString: string): string {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
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

<div class="chat-list">
  <div class="chat-list-header">
    <button
      class="{commonStyles['btn']} {commonStyles['btn-primary']} {commonStyles['btn-sm']}"
      disabled={isCreating || !isConnected}
      onclick={handleCreateChat}
    >
      {isCreating ? 'Creating...' : '+ New Chat'}
    </button>
  </div>

  {#if chatsLoading}
    <div class="chat-list-loading">
      <span class="text-muted">Loading chats...</span>
    </div>
  {:else if chatsError}
    <div class="chat-list-error">
      <span class="text-error">{chatsError}</span>
      <button class="btn-icon" onclick={() => chatsListStore.error = null} title="Dismiss">×</button>
    </div>
  {:else if !hasChats}
    <div class="chat-list-empty">
      <span class="text-muted">No chats yet</span>
    </div>
  {:else}
    <ul class="{commonStyles['list']} chat-list-items">
      {#each chats as chat (chat.id)}
        <li
          class="{commonStyles['list-item']} {selectedChatId === chat.id ? commonStyles['active'] : ''}"
          onclick={() => handleChatClick(chat.id)}
          onkeydown={(e) => e.key === 'Enter' && handleChatClick(chat.id)}
          role="button"
          tabindex="0"
          aria-selected={selectedChatId === chat.id}
        >
          <div class="chat-item-content">
            <div class="chat-item-title truncate">{chat.title}</div>
            <div class="chat-item-meta">
              <span class="text-muted text-sm">{formatTime(chat.updatedAt)}</span>
              {#if chat.tags.length > 0}
                <div class="chat-item-tags">
                  {#each chat.tags.slice(0, 2) as tag}
                    <span class={commonStyles['tag']}>{tag}</span>
                  {/each}
                  {#if chat.tags.length > 2}
                    <span class="text-muted text-sm">+{chat.tags.length - 2}</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if hasChats}
    <div class="chat-list-footer">
      <button
        class="{commonStyles['btn']} {commonStyles['btn-danger']} {commonStyles['btn-sm']}"
        disabled={isDeleting}
        onclick={handleDeleteAllClick}
      >
        {isDeleting ? 'Deleting...' : 'Delete All Chats'}
      </button>
    </div>
  {/if}
</div>

{#if showDeleteAllModal}
  <ConfirmModal
    title="Delete All Chats"
    message="Are you sure you want to delete all chats? This action cannot be undone."
    confirmText="Delete All"
    confirmVariant="danger"
    onConfirm={handleDeleteAllConfirm}
    onCancel={handleDeleteAllCancel}
  />
{/if}

<style>
  .chat-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chat-list-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
  }

  .chat-list-loading,
  .chat-list-empty {
    padding: var(--spacing-lg);
    text-align: center;
  }

  .chat-list-error {
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--color-error-bg);
    border-bottom: 1px solid var(--color-error);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--font-size-sm);
  }

  .btn-icon {
    background: none;
    border: none;
    color: var(--color-error);
    font-size: var(--font-size-lg);
    cursor: pointer;
    padding: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
  }

  .btn-icon:hover {
    background: rgba(0, 0, 0, 0.1);
  }

  .chat-list-items {
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
  }

  .chat-item-tags {
    display: flex;
    gap: var(--spacing-xs);
    align-items: center;
  }

  .text-sm {
    font-size: var(--font-size-xs);
  }

  .chat-list-footer {
    padding: var(--spacing-md);
    border-top: 1px solid var(--color-border);
  }
</style>
