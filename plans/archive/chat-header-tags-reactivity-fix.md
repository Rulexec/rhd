# Chat Header Tags Reactivity Fix

## Problem

Tags in the chat view header are not reactive. When plugins add tags or tags are added via the button, the tags displayed in the chat header do not update.

## Root Cause

1. In [`ChatView.svelte`](frontend/src/lib/components/ChatView.svelte:16), `currentChat` is derived from `mobxObservable(() => chatStore.currentChat)`
2. `ChatStore.currentChat` is set once when [`selectChat()`](frontend/src/stores/ChatStore.ts:209) is called (line 249)
3. When tags are added via [`chatsListStore.addChatTags()`](frontend/src/stores/ChatsListStore.ts:161), it triggers a `chatUpdated` event
4. `ChatsListStore` handles this in [`#handleChatUpdated()`](frontend/src/stores/ChatsListStore.ts:202) and updates its `_chats` array
5. **The bug**: `ChatStore.currentChat` is a separate reference that never gets updated when the chat is modified. There's no synchronization between the two stores.

## Solution

The `ChatStore` needs to listen for `chatUpdated` events for the currently selected chat and update `currentChat` accordingly.

## Implementation Plan

### Step 1: Add chat update event handler to ChatStore

In [`ChatStore.ts`](frontend/src/stores/ChatStore.ts), modify the `selectChat()` method to register a handler for `onChatUpdated` events.

**Current code (lines 222-232):**
```typescript
this.#cleanupEvents = this.#chatApi.onChatEvents(chatId, {
  onMessageAdded: ({ message }) => {
    this.#handleMessageAdded(message);
  },
  onMessageUpdated: ({ message }) => {
    this.#handleMessageUpdated(message);
  },
  onMessageDeleted: ({ messageId }) => {
    this.#handleMessageDeleted(messageId);
  }
});
```

**Updated code:**
```typescript
this.#cleanupEvents = this.#chatApi.onChatEvents(chatId, {
  onChatUpdated: ({ chat }) => {
    this.#handleChatUpdated(chat);
  },
  onMessageAdded: ({ message }) => {
    this.#handleMessageAdded(message);
  },
  onMessageUpdated: ({ message }) => {
    this.#handleMessageUpdated(message);
  },
  onMessageDeleted: ({ messageId }) => {
    this.#handleMessageDeleted(messageId);
  }
});
```

### Step 2: Add #handleChatUpdated method to ChatStore

Add a new private method to handle chat updates:

```typescript
/**
 * Handle chat updated event.
 * Updates currentChat when the chat metadata (title, tags, etc.) changes.
 */
#handleChatUpdated(chat: Chat): void {
  if (this.currentChat && this.currentChat.id === chat.id) {
    this.currentChat = chat;
  }
}
```

### Step 3: Verify the fix

After implementing the changes:
1. The `ChatStore` will now listen for `chatUpdated` events
2. When a chat is updated (e.g., tags are added), `currentChat` will be updated
3. The `mobxObservable` in `ChatView.svelte` will detect the change
4. The UI will re-render with the updated tags

## Files to Modify

- [`frontend/src/stores/ChatStore.ts`](frontend/src/stores/ChatStore.ts) - Add `onChatUpdated` handler and `#handleChatUpdated` method

## Testing

After implementation:
1. Add a tag to a chat via the UI button
2. Verify the tag appears in the chat header immediately
3. Have a plugin add a tag to the current chat
4. Verify the tag appears in the chat header immediately
5. Verify the tag also appears in the chat list (already working via `ChatsListStore`)
