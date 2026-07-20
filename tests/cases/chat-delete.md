# Test Case: Delete Chat

## Description
User deletes a chat from the chat list.

## Preconditions
- At least one chat exists
- WebSocket connected

## Steps
1. User clicks delete button (×) on chat item
2. System shows confirmation dialog
3. User confirms deletion
4. System dispatches `deleteChat` action with chatId
5. System sends request to daemon
6. Daemon deletes chat
7. System removes chat from chats store
8. If deleted chat was current:
   - System sets currentChatId to null
   - System clears messages store

## Expected Results
- Chat removed from list
- If was current chat, no chat selected
- Messages cleared if was current chat

## Actions
- `deleteChat` — dispatched when user confirms deletion

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `deletes chat and updates state` (steps 4-8)

### UI Tests
- [`ChatList.test.ts`](frontend/src/tests/ui/ChatList.test.ts) - `renders delete button for each chat` (step 1)

### Coverage Notes
- Steps 2-3 (confirmation dialog) are not covered by tests
- Partial coverage: steps 1, 4-8 covered (75%)
