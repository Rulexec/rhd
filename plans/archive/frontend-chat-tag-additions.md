# Frontend Chat Tag Additions

## Overview

Add support for adding tags to chats in the frontend. Tags can be added via a dropdown/popover UI in the chat view header, with suggestions aggregated from all existing chats.

**Scope:**
- ✅ Chat tag additions
- ❌ Chat tag removals (not required)
- ❌ Message tag additions (not required)

## Current State

### Backend (Already Implemented)
- [`updateChat`](packages/rhd_chat_server/src/handlers/chat.rs:243) method supports `addTags` parameter
- [`chatUpdated`](packages/rhd_chat_api/src/events/chat_updated.rs:1) event broadcasts tag changes
- Database has [`add_chat_tags()`](packages/rhd_db/src/chat_db/tags.rs:91) function

### Frontend (Needs Implementation)
- No `updateChat` API method in [`ChatApi.ts`](frontend/src/lib/api/ChatApi.ts:1)
- No `updateChat` function in [`chatApiImpl.ts`](frontend/src/lib/api/chatApiImpl.ts:1)
- No `UpdateChatParams`/`UpdateChatResult` schemas in [`schemas.ts`](frontend/src/lib/api/schemas.ts:1)
- [`ChatsListStore.ts`](frontend/src/stores/ChatsListStore.ts:1) handles `chatUpdated` events but has no method to call `updateChat`
- UI components display tags but have no UI for adding tags

## Implementation Plan

### Phase 1: API Layer

#### 1.1 Add Schemas (`frontend/src/lib/api/schemas.ts`)

Add the following schemas:

```typescript
/**
 * Update chat params schema.
 */
export const UpdateChatParamsSchema = z.object({
  chatId: z.number(),
  title: z.string().optional(),
  addTags: z.array(z.string()).default([]),
  removeTags: z.array(z.string()).default([])
});

/**
 * Update chat result schema.
 */
export const UpdateChatResultSchema = z.object({});

// Type exports
export type UpdateChatParams = z.infer<typeof UpdateChatParamsSchema>;
export type UpdateChatResult = z.infer<typeof UpdateChatResultSchema>;
```

#### 1.2 Add API Function (`frontend/src/lib/api/chatApiImpl.ts`)

Add the `updateChat` function:

```typescript
/**
 * Update a chat (title, tags).
 * @param params - Update parameters
 */
export async function updateChat(params: UpdateChatParams): Promise<UpdateChatResult> {
  const data = await websocket.request('updateChat', params);
  return UpdateChatResultSchema.parse(data);
}
```

#### 1.3 Update ChatApi Interface (`frontend/src/lib/api/ChatApi.ts`)

Add `updateChat` to the interface and default implementation:

```typescript
export interface ChatApi {
  // ... existing methods
  updateChat: typeof chatApi.updateChat;
}

export const defaultChatApi: ChatApi = {
  // ... existing methods
  updateChat: chatApi.updateChat,
};
```

### Phase 2: Store Layer

#### 2.1 Add Methods to ChatsListStore (`frontend/src/stores/ChatsListStore.ts`)

Add the following:

```typescript
/**
 * Get all unique tags across all chats (for suggestions).
 */
get allTags(): string[] {
  const tagSet = new Set<string>();
  for (const chat of this._chats) {
    for (const tag of chat.tags) {
      tagSet.add(tag);
    }
  }
  return Array.from(tagSet).sort();
}

/**
 * Add tags to a chat.
 * @param chatId - Chat ID
 * @param tags - Tags to add
 */
*addChatTags(chatId: number, tags: string[]): Generator<unknown, void, unknown> {
  this.error = null;
  try {
    yield* yieldPromise(this.#chatApi.updateChat({
      chatId,
      addTags: tags
    }));
    // Chat will be updated via chatUpdated event
  } catch (error) {
    this.error = error instanceof Error ? error.message : String(error);
  }
}
```

### Phase 3: Component Layer

#### 3.1 Create TagInput Component (`frontend/src/lib/components/TagInput.svelte`)

Create a new component with:
- A '+' button that opens a dropdown/popover
- Text input for filtering/creating tags
- List of suggested tags (from `allTags`) filtered by input
- List of already-applied tags (to exclude from suggestions)
- Click on suggestion adds the tag
- Enter key creates new tag if not in suggestions
- Escape closes the dropdown

**Props:**
- `currentTags: string[]` - Tags already applied to the chat
- `suggestions: string[]` - All available tags for suggestions
- `onAddTag: (tag: string) => void` - Callback when tag is added

**UI Structure:**
```
[Tag 1] [Tag 2] [+] ← Click opens dropdown
                    ↓
                ┌─────────────────┐
                │ [Search input]  │
                │─────────────────│
                │ suggestion-1    │
                │ suggestion-2    │
                │ + Create "new"  │
                └─────────────────┘
```

#### 3.2 Update ChatView Component (`frontend/src/lib/components/ChatView.svelte`)

Modify the chat header to include the TagInput component:

```svelte
<script lang="ts">
  // ... existing imports
  import TagInput from './TagInput.svelte';
  
  const appStore = getAppStore();
  const allTagsGetter = mobxObservable(() => appStore.chatsList.allTags);
  let allTags = $derived(allTagsGetter());
  
  function handleAddTag(tag: string) {
    if (currentChat) {
      flowResult(appStore.chatsList.addChatTags(currentChat.id, [tag]));
    }
  }
</script>

<!-- In the header section -->
<div class="chat-header">
  <h2>{currentChat.title}</h2>
  {#if currentChat}
    <TagInput 
      currentTags={currentChat.tags} 
      suggestions={allTags}
      onAddTag={handleAddTag}
    />
  {/if}
</div>
```

### Phase 4: Testing

#### 4.1 API Tests (`frontend/src/lib/api/chatApiImpl.test.ts`)

Add tests for `updateChat`:
- Test successful tag addition
- Test error handling

#### 4.2 Store Tests (`frontend/src/stores/ChatsListStore.test.ts`)

Add tests for:
- `allTags` getter returns unique sorted tags
- `addChatTags` calls API with correct params
- `addChatTags` handles errors

#### 4.3 Component Tests (`frontend/src/lib/components/TagInput.test.ts`)

Add tests for:
- Dropdown opens on '+' click
- Suggestions filter based on input
- Already-applied tags are excluded from suggestions
- Click on suggestion calls `onAddTag`
- Enter key creates new tag
- Escape closes dropdown

## File Changes Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `frontend/src/lib/api/schemas.ts` | Modify | Add `UpdateChatParamsSchema`, `UpdateChatResultSchema` |
| `frontend/src/lib/api/chatApiImpl.ts` | Modify | Add `updateChat` function |
| `frontend/src/lib/api/ChatApi.ts` | Modify | Add `updateChat` to interface and default |
| `frontend/src/stores/ChatsListStore.ts` | Modify | Add `allTags` getter, `addChatTags` method |
| `frontend/src/lib/components/TagInput.svelte` | Create | New tag input component with dropdown |
| `frontend/src/lib/components/ChatView.svelte` | Modify | Integrate TagInput in header |
| `frontend/src/lib/api/chatApiImpl.test.ts` | Modify | Add tests for `updateChat` |
| `frontend/src/stores/ChatsListStore.test.ts` | Modify | Add tests for new methods |
| `frontend/src/lib/components/TagInput.test.ts` | Create | Add component tests |

## Dependencies

- No new npm dependencies required
- Uses existing MobX patterns and Svelte 5 runes

## Success Criteria

1. User can click '+' button in chat view header to open tag input dropdown
2. Dropdown shows suggestions from all existing chat tags
3. User can filter suggestions by typing
4. User can click a suggestion to add the tag
5. User can type a new tag name and press Enter to create it
6. Tag is added to the chat and UI updates immediately
7. Already-applied tags are not shown in suggestions
8. All tests pass
