# TypeScript Migration Plan

## Overview
Replace all JavaScript in the frontend with TypeScript. This includes converting 6 JS files to TS, adding `lang="ts"` to 12 Svelte components, updating config files, defining type interfaces, and adding Zod schemas for runtime validation of WebSocket messages.

## Files to Migrate

### JavaScript → TypeScript Files
| Current | Target |
|---------|--------|
| `src/main.js` | `src/main.ts` |
| `src/lib/ws.js` | `src/lib/ws.ts` |
| `src/lib/stores.js` | `src/lib/stores.ts` |
| `src/lib/chatStores.js` | `src/lib/chatStores.ts` |
| `src/lib/chatWs.js` | `src/lib/chatWs.ts` |
| `src/lib/utils.js` | `src/lib/utils.ts` |

### Svelte Components (add `lang="ts"` to `<script>`)
- `App.svelte`
- `ScenariosTab.svelte`
- `ChatsTab.svelte`
- `ChatView.svelte`
- `Message.svelte`
- `MessageInput.svelte`
- `ActiveScenario.svelte`
- `FinishedScenario.svelte`
- `ChatList.svelte`
- `MessageList.svelte`
- `StreamingMessage.svelte`
- `TabNav.svelte`

## Type Definitions & Zod Schemas

### Domain Types with Zod (`src/lib/types/index.ts`)
```typescript
import { z } from 'zod';

// Scenario schemas
export const ActiveScenarioSchema = z.object({
  id: z.string(),
  scenarioName: z.string(),
  startedAt: z.string(),
  currentStep: z.string().nullable(),
  currentStepStartedAt: z.string().nullable(),
  promptTokens: z.number(),
  completionTokens: z.number(),
});
export type ActiveScenario = z.infer<typeof ActiveScenarioSchema>;

export const FinishedScenarioSchema = z.object({
  id: z.number(),
  scenario: z.string(),
  status: z.enum(['success', 'error', 'aborted', 'executing']),
  finished: z.string(),
  durationMs: z.number(),
  tokens: z.object({
    promptTokens: z.number(),
    completionTokens: z.number(),
  }).optional(),
  cost: z.number().nullable().optional(),
});
export type FinishedScenario = z.infer<typeof FinishedScenarioSchema>;

// Chat schemas
export const ChatSchema = z.object({
  id: z.number(),
  title: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});
export type Chat = z.infer<typeof ChatSchema>;

export const ChatMessageSchema = z.object({
  id: z.number(),
  chatId: z.number(),
  role: z.enum(['user', 'assistant']),
  content: z.string(),
  createdAt: z.string(),
});
export type ChatMessage = z.infer<typeof ChatMessageSchema>;
```

### WebSocket Protocol Schemas (`src/lib/types/ws.ts`)
```typescript
import { z } from 'zod';
import { ChatMessageSchema, FinishedScenarioSchema } from './index';

// Response schema
export const WsResponseSchema = z.object({
  id: z.string(),
  type: z.literal('response'),
  success: z.boolean(),
  data: z.any().optional(),
  error: z.string().optional(),
});
export type WsResponse = z.infer<typeof WsResponseSchema>;

// Event schemas
export const ScenarioStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenariostarted'),
  data: z.object({
    id: z.string(),
    name: z.string(),
    startedAt: z.string(),
  }),
});

export const StepStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('stepstarted'),
  data: z.object({
    executionId: z.string(),
    stepName: z.string(),
    startedAt: z.string(),
  }),
});

export const ScenarioFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenariofinished'),
  data: FinishedScenarioSchema,
});

export const ChatStreamChunkEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamChunk'),
  data: z.object({
    content: z.string(),
  }),
});

export const ChatStreamFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamFinished'),
  data: z.object({
    messageId: z.number(),
    chatId: z.number(),
  }),
});

export const ChatStreamErrorEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamError'),
  data: z.object({
    error: z.string(),
  }),
});

export const ChatMessageAddedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatMessageAdded'),
  data: z.object({
    message: ChatMessageSchema,
  }),
});

export const ChatUpdatedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatUpdated'),
  data: z.object({
    chatId: z.number(),
    title: z.string(),
  }),
});

export const WsEventSchema = z.discriminatedUnion('event', [
  ScenarioStartedEventSchema,
  StepStartedEventSchema,
  ScenarioFinishedEventSchema,
  ChatStreamChunkEventSchema,
  ChatStreamFinishedEventSchema,
  ChatStreamErrorEventSchema,
  ChatMessageAddedEventSchema,
  ChatUpdatedEventSchema,
]);
export type WsEvent = z.infer<typeof WsEventSchema>;

// Top-level message schema
export const WsMessageSchema = z.discriminatedUnion('type', [
  WsResponseSchema,
  WsEventSchema,
]);
export type WsMessage = z.infer<typeof WsMessageSchema>;
```

## Implementation Steps

### Step 1: Add Dependencies
Update `package.json`:
```json
{
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5.0.3",
    "svelte": "^5.28.2",
    "svelte-check": "^4.1.1",
    "tslib": "^2.8.1",
    "typescript": "^5.7.2",
    "vite": "^6.3.5",
    "zod": "^3.24.1"
  }
}
```

### Step 2: Create `tsconfig.json`
```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": {
    "target": "ESNext",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "resolveJsonModule": true,
    "allowJs": true,
    "checkJs": true,
    "isolatedModules": true,
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
```

### Step 3: Create Type & Schema Files
- Create `src/lib/types/index.ts` with domain types and Zod schemas
- Create `src/lib/types/ws.ts` with WebSocket protocol schemas

### Step 4: Convert JS Files to TS
Convert each file with type annotations and Zod validation:
1. `utils.js` → `utils.ts` (simplest, no dependencies)
2. `stores.js` → `stores.ts` (depends on types)
3. `chatStores.js` → `chatStores.ts` (depends on types)
4. `ws.js` → `ws.ts` (depends on types, stores; **add Zod validation for incoming messages**)
5. `chatWs.js` → `chatWs.ts` (depends on types, stores, ws)
6. `main.js` → `main.ts` (entry point)

**Key validation point in `ws.ts`:**
```typescript
socket.onmessage = (event) => {
  const raw = JSON.parse(event.data);
  const result = WsMessageSchema.safeParse(raw);
  
  if (!result.success) {
    console.error('Invalid WebSocket message:', result.error);
    return;
  }
  
  const message = result.data;
  if (message.type === 'response') {
    // handle response
  } else if (message.type === 'event') {
    handleEvent(message);
  }
};
```

### Step 5: Update Svelte Components
For each component:
1. Add `lang="ts"` to `<script>` tag
2. Update import paths (remove `.js` extension or use `.ts`)
3. Add type annotations to props and variables
4. Fix any type errors

### Step 6: Update Config Files (optional)
- `svelte.config.js` → `svelte.config.ts`
- `vite.config.js` → `vite.config.ts`

### Step 7: Verify Build
Run `npm run build` and `npx svelte-check` to verify no type errors.

## Migration Order

1. **Dependencies** - Add TypeScript and Zod
2. **Config** - Create `tsconfig.json`
3. **Types & Schemas** - Create type definition files with Zod schemas
4. **Utils** - No dependencies, easy to convert
5. **Stores** - Depends on types
6. **Chat stores** - Depends on types
7. **WebSocket client** - Depends on types and stores; **add Zod validation**
8. **Chat WebSocket** - Depends on types, stores, ws
9. **Main entry** - Last, depends on everything
10. **Svelte components** - Can be done in parallel after lib files

## Notes

- Svelte 5 runes (`$state`, `$props`, `$derived`, `$effect`) work with TypeScript
- Use `svelte-check` for type checking Svelte files
- Vite handles TypeScript natively, no additional config needed
- Zod schemas provide runtime validation for all WebSocket messages
- Invalid messages are logged and ignored, preventing runtime errors from malformed data
- Consider using `@tsconfig/svelte` for base TypeScript config
