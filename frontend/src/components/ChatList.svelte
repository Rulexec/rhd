<script>
  import { chats, currentChatId } from '../lib/chatStores.js';
  import { createChat, selectChat, deleteChat } from '../lib/chatWs.js';

  async function handleCreateChat() {
    const title = prompt('Enter chat title:');
    if (title && title.trim()) {
      await createChat(title.trim());
    }
  }

  async function handleDeleteChat(event, chatId) {
    event.stopPropagation();
    if (confirm('Delete this chat?')) {
      await deleteChat(chatId);
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
        on:click={() => selectChat(chat.id)}
        on:keydown={(e) => e.key === 'Enter' && selectChat(chat.id)}
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
</div>

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
</style>
