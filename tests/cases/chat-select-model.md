# Test Case: Select Model

## Description
User selects a model for the current chat.

## Preconditions
- Chat exists and is selected
- Available models loaded

## Steps
1. System loads available models on mount
2. System populates model selector dropdown
3. If no model selected, system auto-selects first model
4. User clicks model selector dropdown
5. User selects a model
6. System dispatches `selectModel` action with model name
7. System updates selectedModel store
8. Model selection persists for current chat

## Expected Results
- Model selector shows available models
- First model auto-selected if none selected
- selectedModel store updated
- Model persists for chat session

## Actions
- `selectModel` — dispatched when user selects model

## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering)
