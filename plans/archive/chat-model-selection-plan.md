# Chat Model Selection Plan

## Overview
Add model selection to chat interface with per-chat model persistence and visual model change indicators in message history.

## Requirements
1. Model selector dropdown under chat message input
2. WS protocol method to get available models (real models only, no aliases)
3. Persist selected model per chat - reopening chat shows same model
4. Visual model indicator entries in chat history when model changes
5. Model indicators are visual-only, not sent to AI

## Architecture

### Data Model Changes

**`chats` table**: Add `active_model TEXT` column
- Stores currently selected model for the chat
- Updated when user changes model selection

**`messages` table**: Add `model TEXT` column  
- Stores which model was used for each message
- Used to detect model changes and render indicators

### Backend Changes

#### 1. Model Config Tracking (`packages/rhd_ai/src/config.rs`)
- Add `is_alias: bool` field to `ModelConfig`
- Set `is_alias = false` for real models, `true` for resolved aliases
- This allows filtering real models from aliases

#### 2. WS Protocol (`packages/rhd_api/src/lib.rs`)
- Add `GetAvailableModels` request variant:
  ```rust
  GetAvailableModels { id: String }
  ```
- Response returns list of real model names (where `is_alias == false`)

#### 3. WS Handler (`packages/rhd_app/src/ws.rs`)
- Add handler for `GetAvailableModels`:
  - Filter `state.models` for entries where `is_alias == false`
  - Return sorted list of model names

#### 4. Chat DB Schema (`packages/rhd_db/src/chat_db.rs`)
- Add migration for `chats` table: `ALTER TABLE chats ADD COLUMN active_model TEXT`
- Add migration for `messages` table: `ALTER TABLE messages ADD COLUMN model TEXT`
- Update `Message` struct to include `model: Option<String>`
- Update `ChatInfo` struct to include `active_model: Option<String>`
- Update `add_message()` to accept and store model parameter
- Update `get_messages()` to return model field
- Add `update_chat_active_model()` method
- Add `get_chat_active_model()` method (or include in `get_chat()`)

#### 5. Chat Manager (`packages/rhd_app/src/chat.rs`)
- Update `send_message()`:
  - Save model with user message
  - Update chat's `active_model`
  - Filter out model indicator entries when building AI context (if using special role)
- Update `edit_and_resend()`:
  - Save model with updated message
- Update `get_chat()` to return `active_model`

#### 6. Chat Events (`packages/rhd_app/src/chat.rs`)
- No new events needed - model info included in existing `MessageAdded` event via `Message` struct

### Frontend Changes

#### 1. Types (`frontend/src/lib/types/index.ts`)
- Update `ChatMessage` schema to include `model: string | null`
- Update `Chat` schema to include `activeModel: string | null`

#### 2. WS Types (`frontend/src/lib/types/ws.ts`)
- Add `GetAvailableModelsResponse` schema
- Update `ChatMessageSchema` to include model field

#### 3. Chat Stores (`frontend/src/lib/chatStores.ts`)
- Add `availableModels` writable store: `string[]`
- Add `selectedModel` writable store: `string | null` (current chat's active model)

#### 4. Chat WS (`frontend/src/lib/chatWs.ts`)
- Add `loadAvailableModels()` function:
  - Send `getAvailableModels` request
  - Populate `availableModels` store
- Update `selectChat()`:
  - Set `selectedModel` from chat's `activeModel`
- Update `sendMessage()`:
  - Use `selectedModel` store value
  - Update `selectedModel` if model changed
- Update message handling to include model field

#### 5. MessageInput Component (`frontend/src/components/MessageInput.svelte`)
- Add model selector dropdown above textarea
- Fetch available models on mount (call `loadAvailableModels()`)
- Bind dropdown to `selectedModel` store
- When model changes:
  - Update chat's active model (send WS request or include in next message)
  - Visual feedback showing current model

#### 6. MessageList Component (`frontend/src/components/MessageList.svelte`)
- Render model indicator entries between messages where model changes
- Logic:
  - Track `previousModel` while iterating messages
  - When `message.model !== previousModel`, render indicator
  - Indicator shows model name (e.g., "Model: gpt4")
- Indicator styling: subtle divider/badge showing model name

#### 7. Message Component (`frontend/src/components/Message.svelte`)
- No changes needed - model info handled at list level

## Implementation Order

1. **Backend: Model config tracking**
   - Add `is_alias` field to `ModelConfig`
   - Update `load_models()` to set flag correctly

2. **Backend: WS protocol for available models**
   - Add `GetAvailableModels` to `WsRequest` enum
   - Implement handler in `ws.rs`

3. **Backend: DB schema migration**
   - Add `active_model` to `chats` table
   - Add `model` to `messages` table
   - Update `Message` and `ChatInfo` structs
   - Update DB methods

4. **Backend: Chat manager updates**
   - Update `send_message()` to save model
   - Update `edit_and_resend()` to save model
   - Update `get_chat()` to return active_model

5. **Frontend: Types and stores**
   - Update Zod schemas
   - Add `availableModels` and `selectedModel` stores

6. **Frontend: WS functions**
   - Add `loadAvailableModels()`
   - Update existing functions for model handling

7. **Frontend: UI components**
   - Add model selector to `MessageInput`
   - Add model indicators to `MessageList`

## Key Design Decisions

1. **Model indicators storage**: Store model with each message, compute indicators on frontend
   - Pros: Simple, no special DB entries, easy to query
   - Cons: Frontend must compute indicators

2. **Filtering for AI context**: When building message history for AI, include all messages (model field not sent to AI)
   - Model indicators are purely visual, AI sees normal conversation

3. **Alias filtering**: Use `is_alias` flag rather than comparing names
   - More explicit, handles edge cases where alias name might match resolved target

## Migration Strategy

SQLite migrations using `ALTER TABLE ADD COLUMN`:
- Existing chats will have `active_model = NULL`
- Existing messages will have `model = NULL`
- Frontend handles NULL gracefully (no indicator shown for old messages)
