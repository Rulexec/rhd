# Delete All Chats Feature

## Overview
Add a "Delete all chats" button at the end of the chats list that removes all chats at once.

## Implementation Plan

### Backend Changes (Rust)

1. **Add `delete_all_chats()` to ChatDb** (`packages/rhd_db/src/chat_db.rs`)
   - Add method that executes `DELETE FROM chats` (CASCADE will handle messages and chat_projects)
   - Add unit test

2. **Add `delete_all_chats()` to ChatManager** (`packages/rhd_chat/src/manager.rs`)
   - Add method that calls `db.delete_all_chats()`
   - Should also clear any active streams from the `active_streams` HashMap

3. **Add `DeleteAllChats` to WsRequest enum** (`packages/rhd_api/src/lib.rs`)
   - Add new variant: `DeleteAllChats { id: String }`
   - Add serialization test

4. **Add handler in ws.rs** (`packages/rhd_app/src/ws.rs`)
   - Add `handle_delete_all_chats()` function
   - Wire it up in the main request match statement

### Frontend Changes (TypeScript/Svelte)

5. **Add action type** (`frontend/src/lib/actions/types.ts`)
   - Add `{ type: 'deleteAllChats' }` to ChatAction union

6. **Add WebSocket function** (`frontend/src/lib/chatWs.ts`)
   - Add `deleteAllChats()` function that sends request and clears the chats store
   - Should also reset `currentChatId`, `messages`, and other related stores

7. **Add processor case** (`frontend/src/lib/actions/processors.ts`)
   - Add case for `deleteAllChats` that calls the new function

8. **Add button to ChatList** (`frontend/src/components/ChatList.svelte`)
   - Add "Delete all chats" button at the bottom of the chat list
   - Show confirmation dialog before deleting
   - Disable button when no chats exist
   - Add appropriate styling

## Files to Modify

| File | Change |
|------|--------|
| `packages/rhd_db/src/chat_db.rs` | Add `delete_all_chats()` method + test |
| `packages/rhd_chat/src/manager.rs` | Add `delete_all_chats()` method |
| `packages/rhd_api/src/lib.rs` | Add `DeleteAllChats` variant to WsRequest |
| `packages/rhd_app/src/ws.rs` | Add handler for delete all chats |
| `frontend/src/lib/actions/types.ts` | Add `deleteAllChats` action type |
| `frontend/src/lib/chatWs.ts` | Add `deleteAllChats()` function |
| `frontend/src/lib/actions/processors.ts` | Add case for `deleteAllChats` |
| `frontend/src/components/ChatList.svelte` | Add button + handler |

## UI Design

The button will be placed at the bottom of the chat list sidebar, below all chat items:
- Label: "Delete all chats"
- Style: Subtle/danger styling (red text or outlined button)
- Confirmation: Browser `confirm()` dialog
- Disabled state: When `$chats.length === 0`
