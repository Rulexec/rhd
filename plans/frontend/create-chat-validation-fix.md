# Create Chat Validation Fix

## Problem

When creating a new chat, the frontend shows a Zod validation error:
```
[ { "code": "invalid_type", "expected": "object", "received": "undefined", "path": [ "chat" ], "message": "Required" } ]
```

But the actual server response is correct:
```json
{"data":{"chatId":11},"id":"...","success":true,"type":"response"}
```

## Root Cause

1. **Schema mismatch**: The backend's `CreateChatResult` (in `packages/rhd_chat_api/src/methods/create_chat.rs`) returns only `{ "chatId": 123 }`, but the frontend schema `CreateChatResultSchema` (in `frontend/src/lib/api/schemas.ts`) expects `{ "chat": ChatSchema }` (a full chat object).

2. **Store code issue**: In `frontend/src/lib/stores/chats.ts:91`, the code tries to access `result.chat` which doesn't exist in the actual response.

3. **Missing error logging**: Zod validation errors are not being printed to the console, making debugging difficult.

## Solution

### 1. Fix `CreateChatResultSchema` in `frontend/src/lib/api/schemas.ts`

Change from:
```typescript
export const CreateChatResultSchema = z.object({
  chat: ChatSchema
});
```

To:
```typescript
export const CreateChatResultSchema = z.object({
  chatId: z.number()
});
```

### 2. Update `CreateChatResult` type export

The type will automatically update since it's derived from the schema.

### 3. Fix `createNewChat()` in `frontend/src/lib/stores/chats.ts`

Change from:
```typescript
const result = await createChat(title);
// Chat will be added via chatCreated event
return result.chat;
```

To:
```typescript
const result = await createChat(title);
// Chat will be added via chatCreated event
// Return null since we only get chatId, not the full chat object
return null;
```

Or alternatively, we could fetch the chat after creation, but since the chat will be added via the `chatCreated` event, returning null is acceptable.

### 4. Add Zod validation error logging in `frontend/src/lib/api/websocket.ts`

In the `handleMessage` method, add console logging for validation errors:

```typescript
try {
  message = WebSocketMessageSchema.parse(parsed);
} catch (error) {
  console.error('WebSocket message validation failed:', error);
  if (error instanceof z.ZodError) {
    console.error('Validation errors:', error.errors);
  }
  console.error('Raw message:', parsed);
  return;
}
```

Also add logging in `chatApi.ts` for method-specific validation errors:

```typescript
export async function createChat(title: string): Promise<CreateChatResult> {
  const data = await websocket.request('createChat', { title });
  try {
    return CreateChatResultSchema.parse(data);
  } catch (error) {
    if (error instanceof z.ZodError) {
      console.error('CreateChat validation errors:', error.errors);
      console.error('Raw data:', data);
    }
    throw error;
  }
}
```

## Files to Modify

1. `frontend/src/lib/api/schemas.ts` - Fix `CreateChatResultSchema`
2. `frontend/src/lib/stores/chats.ts` - Fix `createNewChat()` return value
3. `frontend/src/lib/api/websocket.ts` - Add Zod error logging
4. `frontend/src/lib/api/chatApi.ts` - Add Zod error logging for method-specific validation

## Testing

After implementing the fix:
1. Create a new chat via the UI
2. Verify no validation errors appear in the console
3. Verify the chat is created and appears in the chat list
4. Verify Zod validation errors are logged to console when they occur
