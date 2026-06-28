<script lang="ts">
  import { editMessage } from '../lib/chatWs';
  import type { ChatMessage } from '../lib/types/index';

  export let message: ChatMessage;

  let editing = false;
  let editContent = message.content;
  let model = 'gpt4';

  function startEdit() {
    editing = true;
    editContent = message.content;
  }

  function cancelEdit() {
    editing = false;
    editContent = message.content;
  }

  async function saveEdit() {
    if (editContent.trim() && editContent !== message.content) {
      await editMessage(message.id, editContent.trim(), model);
    }
    editing = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      saveEdit();
    }
    if (event.key === 'Escape') {
      cancelEdit();
    }
  }
</script>

<div class="message {message.role}">
  {#if editing}
    <div class="edit-mode">
      <textarea
        bind:value={editContent}
        on:keydown={handleKeydown}
        class="edit-textarea"
      ></textarea>
      <div class="edit-actions">
        <button on:click={saveEdit} class="save-btn">Save</button>
        <button on:click={cancelEdit} class="cancel-btn">Cancel</button>
        <span class="hint">Ctrl+Enter to save, Esc to cancel</span>
      </div>
    </div>
  {:else}
    <div class="content">{message.content}</div>
    {#if message.role === 'user'}
      <button class="edit-btn" on:click={startEdit} aria-label="Edit message">
        ✎
      </button>
    {/if}
  {/if}
</div>

<style>
  .message {
    margin-bottom: var(--spacing-m);
    padding: var(--spacing-m);
    border-radius: 8px;
    position: relative;
  }

  .message.user {
    background: var(--color-bg-active);
    margin-left: 40px;
  }

  .message.assistant {
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    margin-right: 40px;
  }

  .content {
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.6;
  }

  .edit-mode {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-s);
  }

  .edit-textarea {
    width: 100%;
    min-height: 80px;
    padding: var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 14px;
    resize: vertical;
  }

  .edit-actions {
    display: flex;
    gap: var(--spacing-s);
    align-items: center;
  }

  .save-btn,
  .cancel-btn {
    padding: var(--spacing-xs) var(--spacing-m);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .save-btn {
    background: var(--color-primary);
    color: white;
  }

  .cancel-btn {
    background: var(--color-border);
    color: var(--color-text);
  }

  .hint {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .edit-btn {
    position: absolute;
    top: var(--spacing-xs);
    right: var(--spacing-xs);
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    font-size: 16px;
    padding: var(--spacing-xs);
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .message.user:hover .edit-btn {
    opacity: 1;
  }

  .edit-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--color-text);
  }
</style>
