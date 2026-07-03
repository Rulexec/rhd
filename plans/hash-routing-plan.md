# Hash-Based Routing Plan

## Goal
Persist active tab and selected chat in URL hash so page reload restores state.

## Hash Format
```
#chats/123    → Chats tab, chat ID 123 selected
#chats        → Chats tab, no chat selected
#scenarios    → Scenarios tab (default)
(empty)       → Scenarios tab
```

## Implementation Steps

### 1. Create `frontend/src/lib/router.ts`
New module with:
- `parseHash()` → `{ tab, chatId }` — parses `window.location.hash`
- `updateHash(tab, chatId?)` — sets `window.location.hash`
- `initRouter()` — listens to `hashchange` event, syncs stores
- `syncHashFromStores()` — called when tab/chatId changes, updates hash

### 2. Modify `frontend/src/App.svelte`
- Import `initRouter`, `parseHash` from router
- On mount: parse hash, set initial `activeTab`
- Subscribe to `activeTab` changes → call `updateHash`
- Call `initRouter()` to listen for hash changes (browser back/forward)

### 3. Modify `frontend/src/lib/chatStores.ts` or `chatWs.ts`
- Subscribe to `currentChatId` changes → update hash with chat ID
- On app init: if hash contains chat ID, call `selectChat()` after chats loaded

### 4. Modify `frontend/src/components/ChatsTab.svelte`
- After `loadChats()` resolves, check hash for chat ID
- If hash has chat ID, call `selectChat(chatId)` to restore chat

## Data Flow
```
Page load → parseHash() → set activeTab, queue selectChat
loadChats() completes → if hash had chatId → selectChat(chatId)
User clicks tab → updateHash(tab)
User selects chat → updateHash(tab, chatId)
Browser back/forward → hashchange → parseHash() → update stores
```

## Files Changed
| File | Change |
|------|--------|
| `frontend/src/lib/router.ts` | **New** — hash parsing, updating, hashchange listener |
| `frontend/src/App.svelte` | Init router, sync tab with hash |
| `frontend/src/components/ChatsTab.svelte` | Restore chat from hash after loadChats |
| `frontend/src/lib/chatWs.ts` | Update hash when currentChatId changes |
