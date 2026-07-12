# Test Case: Create Chat

## Description
User creates a new chat with a title.

## Preconditions
- WebSocket connected to daemon
- No chat currently selected (or any chat can be selected)

## Steps
1. User clicks "+ New Chat" button
2. Dialog opens with title input field
3. User enters chat title
4. User clicks "Create" button
5. System dispatches `createChat` action with title
6. System sends request to daemon
7. Daemon creates chat and returns chatId
8. System updates chats store with new chat
9. System sets currentChatId to new chat
10. Dialog closes

## Expected Results
- New chat appears in chat list
- New chat is selected (currentChatId set)
- Messages store is empty
- Dialog is closed

## Actions
- `createChat` — dispatched when user clicks Create

## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/ChatList.test.ts` (UI rendering)
