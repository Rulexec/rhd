# Phase 3: Frontend — `rhd_choice` Detection & ChoicePrompt UI

> Parent plan: [`plans/rhd-plugin-choice-plan.md`](../rhd-plugin-choice-plan.md)

## Overview

Render `rhd_choice` tool calls in assistant messages as an interactive card: the
assistant's `question`, one **button per option**, and a **free-text input** for a
manual decision. On user action, answer the tool call via the Phase 2 store flow
(`chatStore.addToolResult`). Once answered (a `role:"tool"` message with the
matching `toolCallId` exists — already surfaced by `ChatStore.toolResults`), the
card flips to a resolved state showing the answer.

**Scope in:** schema constant + zod args schema, `ChoicePrompt.svelte`,
`Message.svelte` detection/wiring, `ChatView.svelte` handler, component tests.
**Scope out:** API/store plumbing (Phase 2), backend (Phase 1).

**Dependencies:** Phase 2 (`chatStore.addToolResult` must exist). Needs Phase 1
only for the frozen tool contract. Blocks Phase 4.

## Tool Contract (consumed — from Phase 1)

- Tool name: `rhd_choice`
- `function.arguments` JSON: `{ "question": string, "options": string[] }`
- Answer content sent to the server: the **exact text** of the clicked option, or
  the **trimmed typed text** from the manual input.

## Files to Modify/Create

### 1. `frontend/src/lib/api/schemas.ts` (modify)

**Add** to the "Well-Known State Schemas" section (after `MCP_STATUS_SCHEMA`,
line ~421) — rename the section comment to "Well-Known Schemas & Constants" if
preferred, but keep the anchor style:

```ts
/** Tool name provided by rhd_plugin_choice (frontend answers its calls). */
export const CHOICE_TOOL_NAME = 'rhd_choice';

/**
 * Parsed `function.arguments` of an rhd_choice tool call.
 * Defensive: callers must treat parse failures as "not a choice prompt".
 */
export const ChoiceToolArgsSchema = z.object({
  question: z.string(),
  options: z.array(z.string()).min(1)
});
```

**Add** to the Type Exports section:

```ts
export type ChoiceToolArgs = z.infer<typeof ChoiceToolArgsSchema>;
```

### 2. `frontend/src/lib/components/ChoicePrompt.svelte` (new)

Presentational component (props/callback only — no store access, so tests render
it directly without a harness):

```svelte
<script lang="ts">
  import type { ChoiceToolArgs } from '../api/schemas.js';

  interface Props {
    args: ChoiceToolArgs;
    /** True once a tool-role message with this call's id exists. */
    resolved: boolean;
    /** The answer text (from toolResults) when resolved. */
    answer?: string | null;
    /** Disable interaction (e.g. no responder wired). */
    disabled?: boolean;
    /** Send the answer: exact option text or the user's typed message. */
    onRespond: (content: string) => void;
  }

  let { args, resolved, answer = null, disabled = false, onRespond }: Props = $props();

  let manualText: string = $state('');
  // Optimistic guard: disable controls immediately after the first submit;
  // the `resolved` prop (driven by the messageAdded round-trip) is the final truth.
  let sentLocally: boolean = $state(false);

  let controlsDisabled: boolean = $derived(resolved || sentLocally || disabled);

  function chooseOption(option: string): void {
    if (controlsDisabled) return;
    sentLocally = true;
    onRespond(option);
  }

  function submitManual(): void {
    const text = manualText.trim();
    if (!text || controlsDisabled) return;
    sentLocally = true;
    onRespond(text);
    manualText = '';
  }
</script>

<div class="choice-prompt" class:resolved>
  <div class="choice-question">{args.question}</div>

  {#if resolved}
    <div class="choice-answer">
      <span class="choice-answer-label">Answered:</span>
      <span class="choice-answer-text">{answer ?? ''}</span>
    </div>
  {:else}
    <div class="choice-options">
      {#each args.options as option}
        <button class="choice-option" disabled={controlsDisabled} onclick={() => chooseOption(option)}>
          {option}
        </button>
      {/each}
    </div>

    <form class="choice-manual" onsubmit={(e) => { e.preventDefault(); submitManual(); }}>
      <input
        type="text"
        class="choice-manual-input"
        placeholder="Or type your own answer..."
        bind:value={manualText}
        disabled={controlsDisabled}
        aria-label="Manual answer"
      />
      <button
        type="submit"
        class="choice-manual-submit"
        disabled={controlsDisabled || manualText.trim().length === 0}
      >
        Send
      </button>
    </form>
  {/if}
</div>

<style>
  /* Card: accent left border, bg-tertiary surface, spacing via CSS vars —
     follow ToolCallMessage.svelte's token usage (--color-border,
     --color-bg-secondary/tertiary, --radius-sm, --font-size-sm, --spacing-*).
     .choice-option: pill buttons; .choice-option:hover: bg-secondary.
     .resolved: muted answer text (--color-success for the label). */
</style>
```

### 3. `frontend/src/lib/components/Message.svelte` (modify)

**a. Imports** (top of `<script>`): add

```ts
import ChoicePrompt from './ChoicePrompt.svelte';
import { CHOICE_TOOL_NAME, ChoiceToolArgsSchema, type ChoiceToolArgs } from '../api/schemas.js';
```

**b. Props** (lines 13–20): add the responder callback:

```ts
  interface Props {
    message: MessageType;
    isQueue?: boolean;
    streamContent?: StreamContent | null;
    toolResults?: Map<string, string>;
    /** Called when the user answers an rhd_choice tool call. */
    onChoiceRespond?: (toolCallId: string, content: string) => void;
  }
```

and destructure with `onChoiceRespond = undefined` default.

**c. Detection helpers** (after `displayToolCalls`, line ~59):

```ts
  // Tool-call name across persisted (ToolCall.function.name) and streaming
  // (StreamToolCallDelta.name) shapes — same accessor pattern as
  // ToolCallMessage.svelte.
  function toolCallName(toolCall: ToolCall | StreamToolCallDelta): string {
    return 'function' in toolCall ? toolCall.function.name : toolCall.name;
  }

  function parseChoiceArgs(toolCall: ToolCall | StreamToolCallDelta): ChoiceToolArgs | null {
    if (toolCallName(toolCall) !== CHOICE_TOOL_NAME) return null;
    const raw = 'function' in toolCall ? toolCall.function.arguments : toolCall.arguments;
    try {
      const parsed = ChoiceToolArgsSchema.safeParse(JSON.parse(raw));
      return parsed.success ? parsed.data : null;
    } catch {
      return null; // partial/invalid JSON → fall back to the generic view
    }
  }

  // Interactive only for finalized messages: streamed deltas may carry partial
  // arguments JSON.
  interface ToolCallView {
    toolCall: ToolCall | StreamToolCallDelta;
    choiceArgs: ChoiceToolArgs | null;
  }
  let toolCallViews: ToolCallView[] = $derived(
    displayToolCalls.map((toolCall) => ({
      toolCall,
      choiceArgs: isStreaming ? null : parseChoiceArgs(toolCall)
    }))
  );
```

**d. Template** — replace the `{#each displayToolCalls as toolCall}` block
(lines 212–222) so a parseable `rhd_choice` renders the prompt **instead of** the
generic collapsible (the resolved answer replaces raw-args display; malformed
args keep the generic view):

```svelte
  {:else if toolCallViews.length > 0}
    <!-- Assistant message with tool calls -->
    <div class="tool-calls">
      {#each toolCallViews as view (view.toolCall.id)}
        {#if view.choiceArgs}
          <ChoicePrompt
            args={view.choiceArgs}
            resolved={toolResults.has(view.toolCall.id)}
            answer={toolResults.get(view.toolCall.id) ?? null}
            disabled={onChoiceRespond === undefined}
            onRespond={(content) => onChoiceRespond?.(view.toolCall.id, content)}
          />
        {:else}
          <ToolCallMessage toolCall={view.toolCall} result={toolResults.get(view.toolCall.id) ?? null} />
        {/if}
      {/each}
    </div>
  {/if}
```

(`toolResults` prop already exists — `Message.svelte:20`.)

### 4. `frontend/src/lib/components/ChatView.svelte` (modify)

**Add** handler next to `handleMessageSent` (line ~88):

```ts
  /**
   * Answer an rhd_choice tool call on the user's behalf (Phase 2 store flow).
   * Store sets chatStore.error on failure; the error banner surfaces it and the
   * card stays interactive for a retry (resolved flips only when the tool
   * message arrives back via messageAdded).
   */
  function handleChoiceRespond(toolCallId: string, content: string): void {
    if (!currentChat) return;
    flowResult(chatStore.addToolResult(currentChat.id, toolCallId, content)).catch(() => {
      /* error already recorded in chatStore.error */
    });
  }
```

**Modify** the `<Message .../>` call (line 177) — add the prop:

```svelte
              <Message
                {message}
                isQueue={message.isQueue}
                streamContent={message.isStreaming ? streamContent : null}
                {toolResults}
                onChoiceRespond={handleChoiceRespond}
              />
```

## Tests

### `frontend/src/lib/components/ChoicePrompt.test.ts` (new)

Render the component directly (props-driven, no context needed):

```ts
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChoicePrompt from './ChoicePrompt.svelte';
import type { ChoiceToolArgs } from '../api/schemas.js';

const args: ChoiceToolArgs = { question: 'Which plan?', options: ['Alpha', 'Beta'] };

afterEach(cleanup);
```

Cases:
1. renders question text and one button per option (`getAllByRole('button')`
   filtered — the manual "Send" button is also a button; assert by text).
2. clicking an option calls `onRespond` with the **exact** option text.
3. typing + submitting the form calls `onRespond` with the trimmed text; empty
   input does not call it.
4. after responding, controls are disabled (double-click cannot send twice —
   `sentLocally` guard).
5. `resolved: true` renders the answer text and **no** option buttons.
6. `disabled: true` prevents `onRespond`.

### `frontend/src/lib/components/Message.test.ts` (new)

Build `Message` fixtures like `ChatStore.test.ts` does (`toolCalls: [{ id, type:
'function', function: { name, arguments }, tags: [] }]`, `role: 'assistant'`,
`isFinished: true`, `isStreaming: false`):

1. **choice detection**: `rhd_choice` call with valid args → question text and
   option buttons present; `onChoiceRespond` invoked with the call's id + clicked
   option text.
2. **resolved**: `toolResults = new Map([['call_1', 'Alpha']])` → "Answered:
   Alpha" shown, no buttons.
3. **malformed args**: `arguments: '{"question":'` → generic `ToolCallMessage`
   fallback (`.tool-call` element present, no `.choice-prompt`).
4. **streaming**: `isStreaming: true` message + `streamContent` with
   `isFinished: false` and a `rhd_choice` delta → no `.choice-prompt`
   (generic view only).
5. **non-choice tool**: `get_weather` call → unchanged generic rendering.

## Verification

```bash
cd frontend && npm run check && npm test
```

## Implementation Notes

1. **No new client state for "answered".** `ChatStore.toolResults` (derived from
   `role:"tool"` messages) is the single source of truth; the local `sentLocally`
   guard only covers the request round-trip. If the send fails, the store rethrows
   and sets `error`; the card stays interactive because `resolved` never flips.
2. **Streaming safety**: during streaming, deltas merge partial `arguments`
   strings (`ChatStore.#handleStreamChunk`); rendering interactive buttons from
   half-arrived JSON would flicker options. Gating on `!isStreaming` (the
   existing derived flag) avoids this entirely; the generic collapsible already
   shows partial args during streaming.
3. **Multiple choice calls per message** are independent cards keyed by
   `toolCall.id` in the `#each`.
4. **Queue messages** (`isQueue`) render through the same `Message` component;
   queue messages never carry `rhd_choice` calls (they're user input), so no
   special-casing.
5. **`disabled={onChoiceRespond === undefined}`** keeps `Message` usable in
   contexts without a responder (e.g. future read-only views) without throwing.
6. **Accessibility**: option buttons are real `<button>`s; the manual input has
   `aria-label`; form submit is keyboard-friendly (Enter).

## Dependencies

- Depends on: Phase 2 (`chatStore.addToolResult`); Phase 1's contract only
  (name/args shape), not its code.
- Blocks: Phase 4.
