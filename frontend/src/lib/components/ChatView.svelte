<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { flowResult } from 'mobx';
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import Message from './Message.svelte';
  import MessageInput from './MessageInput.svelte';
  import TagInput from './TagInput.svelte';
  import ChatTabNav from './ChatTabNav.svelte';
  import ToolsList from './ToolsList.svelte';

  const appStore = getAppStore();
  const chatStore = appStore.chat;
  const chatsListStore = appStore.chatsList;

  // Bridge MobX observables to Svelte reactivity.
  // mobxObservable returns a getter and must be invoked at component top level.
  const currentChatGetter = mobxObservable(() => chatStore.currentChat);
  const allMessagesGetter = mobxObservable(() => chatStore.allMessages);
  const chatLoadingGetter = mobxObservable(() => chatStore.loading);
  const chatErrorGetter = mobxObservable(() => chatStore.error);
  const streamContentGetter = mobxObservable(() => chatStore.streamContent);
  const allTagsGetter = mobxObservable(() => chatsListStore.allTags);
  const toolsGetter = mobxObservable(() => chatStore.tools);
  const toolResultsGetter = mobxObservable(() => chatStore.toolResults);

  let currentChat = $derived(currentChatGetter());
  let allMessages = $derived(allMessagesGetter());
  let chatLoading = $derived(chatLoadingGetter());
  let chatError = $derived(chatErrorGetter());
  let streamContent = $derived(streamContentGetter());
  let allTags = $derived(allTagsGetter());
  let tools = $derived(toolsGetter());
  let toolResults = $derived(toolResultsGetter());

  // Tab state
  let activeTab: 'messages' | 'tools' = $state('messages');

  let disposeChatStore: (() => void) | null = null;

  function dismissError() {
    chatStore.error = null;
  }

  let messagesContainer: HTMLDivElement | null = $state(null);
  let isAtBottom: boolean = $state(true);

  onMount(() => {
    disposeChatStore = chatStore.init();
  });

  onDestroy(() => {
    if (disposeChatStore) {
      disposeChatStore();
      disposeChatStore = null;
    }
  });

  /**
   * Check if user is at the bottom of the message list.
   */
  function checkIfAtBottom(): boolean {
    if (!messagesContainer) return false;
    const threshold = 30;
    const { scrollTop, scrollHeight, clientHeight } = messagesContainer;
    return scrollHeight - scrollTop - clientHeight < threshold;
  }

  /**
   * Scroll to bottom of message list.
   */
  function scrollToBottom(): void {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }

  /**
   * Handle scroll event.
   */
  function handleScroll(): void {
    isAtBottom = checkIfAtBottom();
  }

  /**
   * Handle message sent event.
   */
  function handleMessageSent(): void {
    // Scroll to bottom after sending
    setTimeout(() => scrollToBottom(), 0);
  }

  /**
   * Answer an rhd_choice tool call on the user's behalf (Phase 2 store flow).
   * Store sets chatStore.error on failure; the error banner surfaces it and the
   * card stays interactive for a retry (resolved flips only when the tool
   * message arrives back via messageAdded).
   */
  function handleChoiceRespond(toolCallId: string, content: string): void {
    if (!currentChat) return;
    flowResult(chatStore.addToolResult(currentChat.id, toolCallId, content)).catch(() => {
      /* error already recorded in chatStore.error */
    });
  }

  /**
   * Handle adding a tag to the current chat.
   */
  function handleAddTag(tag: string): void {
    if (currentChat) {
      flowResult(chatsListStore.addChatTags(currentChat.id, [tag]));
    }
  }

  // Auto-scroll when messages change (only if at bottom)
  $effect(() => {
    if (allMessages && isAtBottom) {
      // Use setTimeout to ensure DOM is updated
      setTimeout(() => {
        if (isAtBottom) {
          scrollToBottom();
        }
      }, 0);
    }
  });

  // Scroll to bottom when chat loads
  $effect(() => {
    if (allMessages && !chatLoading) {
      setTimeout(() => scrollToBottom(), 0);
    }
  });

  // Auto-scroll when streaming content updates
  $effect(() => {
    if (streamContent && !streamContent.isFinished && isAtBottom) {
      setTimeout(() => {
        if (isAtBottom) {
          scrollToBottom();
        }
      }, 0);
    }
  });
</script>

<div class="chat-view">
  {#if chatLoading}
    <div class="chat-loading">
      <span class="text-muted">Loading chat...</span>
    </div>
  {:else if chatError}
    <div class="chat-error">
      <div class="error-content">
        <span class="text-error">{chatError}</span>
        <button class="btn-icon" onclick={dismissError} title="Dismiss">×</button>
      </div>
    </div>
  {:else if !currentChat}
    <div class="chat-empty">
      <span class="text-muted">No chat selected</span>
    </div>
  {:else}
    <div class="chat-header">
      <h2>{currentChat.title}</h2>
      <TagInput
        currentTags={currentChat.tags}
        suggestions={allTags}
        onAddTag={handleAddTag}
      />
    </div>

    <ChatTabNav
      bind:activeTab
      toolsCount={tools.length}
    />

    {#if activeTab === 'messages'}
      <div
        class="messages-container"
        bind:this={messagesContainer}
        onscroll={handleScroll}
      >
        {#if allMessages.length === 0}
          <div class="messages-empty">
            <span class="text-muted">No messages yet</span>
          </div>
        {:else}
          <div class="messages-list">
            {#each allMessages as message (message.id)}
              <Message
                {message}
                isQueue={message.isQueue}
                streamContent={message.isStreaming ? streamContent : null}
                {toolResults}
                onChoiceRespond={handleChoiceRespond}
              />
            {/each}
          </div>
        {/if}
      </div>

      <MessageInput onMessageSent={handleMessageSent} />
    {:else if activeTab === 'tools'}
      <ToolsList {tools} />
    {/if}
  {/if}
</div>

<style>
  .chat-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .chat-loading,
  .chat-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .chat-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: var(--spacing-lg);
  }

  .error-content {
    padding: var(--spacing-md);
    background: var(--color-error-bg);
    border: 1px solid var(--color-error);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
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

  .chat-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chat-header h2 {
    margin: 0 0 var(--spacing-xs) 0;
    font-size: var(--font-size-lg);
  }

  .messages-container {
    flex: 1;
    overflow-y: auto;
  }

  .messages-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .messages-list {
    display: flex;
    flex-direction: column;
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
