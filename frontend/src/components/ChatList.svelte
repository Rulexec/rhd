<script lang="ts">
  import { chats, currentChatId } from '@/lib/chatStores';
  import { dispatch } from '@/lib/actions';

  let showDialog = false;
  let chatTitle = '';
  let dialogEl: HTMLDialogElement;
  let titleInput: HTMLInputElement;

  function handleCreateChat() {
    chatTitle = '';
    showDialog = true;
    dialogEl.showModal();
    titleInput.focus();
  }

  async function handleSubmit() {
    const trimmed = chatTitle.trim();
    if (trimmed) {
      await dispatch({ type: 'createChat', payload: { title: trimmed } });
      dialogEl.close();
    }
  }

  function handleCancel() {
    dialogEl.close();
  }

  function handleDialogClose() {
    showDialog = false;
    chatTitle = '';
  }

  async function handleDeleteChat(event: Event, chatId: number) {
    event.stopPropagation();
    if (confirm('Delete this chat?')) {
      await dispatch({ type: 'deleteChat', payload: { chatId } });
    }
  }

  async function handleDeleteAllChats() {
    if (confirm('Delete all chats? This action cannot be undone.')) {
      await dispatch({ type: 'deleteAllChats' });
    }
  }
</script>

<div class="chat-list">
  <button class="new-chat-btn" on:click={handleCreateChat}>+ New Chat</button>
  <div class="chats">
    {#each $chats as chat (chat.id)}
      <div
        class="chat-item"
        class:selected={chat.id === $currentChatId}
        on:click={() => dispatch({ type: 'selectChat', payload: { chatId: chat.id } })}
        on:keydown={(e) => e.key === 'Enter' && dispatch({ type: 'selectChat', payload: { chatId: chat.id } })}
        role="button"
        tabindex="0"
      >
        <span class="title">{chat.title}</span>
        <button
          class="delete-btn"
          on:click={(event) => handleDeleteChat(event, chat.id)}
          aria-label="Delete chat"
        >
          ×
        </button>
      </div>
    {/each}
  </div>
  {#if $chats.length > 0}
    <button class="delete-all-btn" on:click={handleDeleteAllChats}>
      Delete all chats
    </button>
  {/if}
</div>

<dialog bind:this={dialogEl} class="chat-dialog" on:close={handleDialogClose}>
  <form method="dialog" on:submit|preventDefault={handleSubmit}>
    <h3>New Chat</h3>
    <input
      type="text"
      bind:this={titleInput}
      bind:value={chatTitle}
      placeholder="Enter chat title"
    />
    <div class="dialog-actions">
      <button type="button" on:click={handleCancel}>Cancel</button>
      <button type="submit" disabled={!chatTitle.trim()}>Create</button>
    </div>
  </form>
</dialog>

<style>
  .chat-list {
    width: 250px;
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    background: var(--color-bg);
  }

  .new-chat-btn {
    margin: var(--spacing-m);
    padding: var(--spacing-s) var(--spacing-m);
    background: var(--color-primary);
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
  }

  .new-chat-btn:hover {
    opacity: 0.9;
  }

  .chats {
    flex: 1;
    overflow-y: auto;
  }

  .chat-item {
    padding: var(--spacing-m);
    border-bottom: 1px solid var(--color-border);
    cursor: pointer;
    display: flex;
    justify-content: space-between;
    align-items: center;
    transition: background 0.2s;
  }

  .chat-item:hover {
    background: var(--color-bg-active);
  }

  .chat-item.selected {
    background: var(--color-bg-active);
    border-left: 3px solid var(--color-primary);
  }

  .title {
    flex: 1;
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .delete-btn {
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 20px;
    cursor: pointer;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .delete-btn:hover {
    background: rgba(0, 0, 0, 0.1);
    color: var(--color-text);
  }

  .delete-all-btn {
    margin: var(--spacing-m);
    padding: var(--spacing-s) var(--spacing-m);
    background: none;
    color: var(--color-danger, #dc3545);
    border: 1px solid var(--color-danger, #dc3545);
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s;
  }

  .delete-all-btn:hover {
    background: var(--color-danger, #dc3545);
    color: white;
  }

  .chat-dialog {
    border: none;
    border-radius: 8px;
    padding: var(--spacing-xl);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
    max-width: 400px;
    width: 90%;
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    margin: 0;
  }

  .chat-dialog::backdrop {
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(2px);
  }

  .chat-dialog h3 {
    margin: 0 0 var(--spacing-m) 0;
    font-size: 18px;
    font-weight: 600;
  }

  .chat-dialog input {
    width: 100%;
    padding: var(--spacing-s) var(--spacing-m);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-size: 14px;
    margin-bottom: var(--spacing-l);
  }

  .chat-dialog input:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-s);
  }

  .dialog-actions button {
    padding: var(--spacing-s) var(--spacing-m);
    border-radius: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  .dialog-actions button[type="button"] {
    background: none;
    border: 1px solid var(--color-border);
    color: var(--color-text);
  }

  .dialog-actions button[type="submit"] {
    background: var(--color-primary);
    color: white;
    border: none;
  }

  .dialog-actions button[type="submit"]:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
