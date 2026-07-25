# Test Case: Storybook Chat Interaction

## Description
User interacts with ChatView through Storybook step-by-step execution controls, verifying the complete message send and receive flow.

## Preconditions
- Storybook is running on port 6006
- ChatView story with controls is loaded
- Mock WebSocket handler is configured

## Steps
1. User navigates to the ChatView story with controls
2. User clicks step 1 button ("Add user message")
3. System sets up mock WebSocket handler for sendMessage
4. System types "Hello, how are you?" into message input
5. System clicks Send button
6. System dispatches sendMessage action
7. User message appears in chat
8. Pause and Abort buttons become visible (streaming state)
9. User clicks step 2 button ("Receive AI response")
10. System emits chatStreamChunk events with AI response content
11. System emits chatStreamFinished event
12. AI message appears in chat with full content
13. Streaming dots disappear
14. Send button becomes disabled (input is empty)

## Expected Results
- Step buttons are visible and clickable
- User message appears after step 1
- Pause/Abort buttons visible during streaming
- AI response appears after step 2
- Streaming indicators removed after completion
- Send button state reflects input state

## Actions
- `sendMessage` — dispatched when user clicks Send
- `chatStreamChunk` — received from WebSocket (emitted by mock)
- `chatStreamFinished` — received from WebSocket (emitted by mock)

## Covered By

### Storybook Tests
- [`chatview.spec.ts`](../../frontend/tests/storybook/chatview.spec.ts) - `step execution flow` (steps 1-14)
