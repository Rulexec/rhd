# Phase 7: Documentation — Request Fidelity Rules (milestone P9)

## Overview

Update the two plugin READMEs to document the behavior shipped by phases 1–6:

- how the AI request is constructed and what the "complete or not sent" principle means,
- the "no request while tool calls are unresolved" trigger gate,
- chat parking on conversion errors and on startup crash detection,
- the new `sendReasoningContent` model flag,
- how another plugin creates a `tool` message (`addMessage` with `role: "tool"` +
  `toolCallId`), and that per-call `tags` never reach the provider,
- that repairing corrupted chats is **not** implemented yet.

Docs-only phase; no code changes. Run after phases 1–6 so the text matches reality.

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/README.md`

**Fix the stale "Error Messages" section** (lines 59–67). It currently claims error
messages have **role** `ai_completions:error` — wrong: the plugin keeps role `assistant`
and adds the `ai_completions:error` **tag** ([`ai_request.rs:301`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:301),
[`ai_request.rs:415`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:415)).
Replace with:

```markdown
## Messages Added

### Error Messages

When an AI request fails, the assistant message is updated with:
- **Role**: `assistant` (unchanged)
- **Tags**: `ai_completions:error` added to the message
- **Content**: Error details (error message, timeout info, etc.)

**Filtering**: Messages tagged `ai_completions:error` are excluded from AI requests by
policy — they are internal bookkeeping, never model output. Messages with unknown roles
are excluded as well. These two are the *only* permitted exclusions; nothing else is ever
silently dropped (see "Request Construction").
```

**Add a new "Request Construction" section** after "Trigger Conditions":

```markdown
## Request Construction

The request sent to the provider is a **faithful rendering of the stored chat history**.
Core principle: **a request is either complete or it is not sent.**

- Assistant `tool_calls` are forwarded field-for-field (`id`, `type`, `function.name`,
  `function.arguments`). Per-call `tags` are RHD-internal orchestration metadata and
  never reach the provider.
- `tool`-role messages are sent with their `tool_call_id`.
- Assistant `reasoning_content` is sent when stored, non-empty, and enabled for the
  model (see `sendReasoningContent`).
- Excluded by policy: messages with unknown roles, and messages tagged
  `ai_completions:error`.

### Integrity validation

Before sending, the tool-call sequence is validated:

- every assistant `tool_calls[].id` is answered by a following `tool` message,
- every `tool` message carries a `tool_call_id` that was previously declared,
- every assistant message has content, tool calls, or reasoning.

A violation means the trigger gate was bypassed or raced (e.g. another plugin deleted a
message between the decision and the build). The plugin then **sends nothing**, adds the
`ai_completions:error` tag to the chat (parking it, same as a failed response), and logs
the offending message and tool-call id. There are no partial requests, ever.

### Unresolved tool calls: no request, keep waiting

While the last assistant message has tool calls without matching results, the plugin does
not trigger at all (`TriggerReason::None`). This is a conversation in progress, not a
malformed request — the plugin keeps polling until a tool-executing plugin posts the
results, then triggers normally.

> Note: no tool-executing plugin exists yet, so a chat whose assistant message has tool
> calls will simply stop triggering. That is correct, safe behavior.
```

**Add startup behavior to the `ai_completions:error` tag section** (lines 49–57):

```markdown
### `ai_completions:error`

Added to a chat when:
1. an AI request fails, or
2. message conversion refuses to build a request (history inconsistency), or
3. at plugin startup, a chat contains a message left unfinished by a crash
   (`isStreaming` or not `isFinished`) — its partial content must never be replayed.

**When**: see above.
**Effect**: the plugin skips chats with this tag (no further processing). The tag is
deliberately not auto-cleared; **repairing crashed chats is not implemented yet**.
```

**Add the config flag** to "Configuration Fields" (after `model`, line 98):

```markdown
    - `sendReasoningContent`: Optional. Replay assistant `reasoning_content` to the
      provider. Default `true`. Set `false` for providers that reject or mishandle it
      (e.g. DeepSeek-R1).
```

And show it in the example YAML:

```yaml
ai_completions:
  models:
    default:
      alias: qwen
    qwen:
      baseUrl: "https://api.example.com/v1"
      apiKey:
        cred: alibabaApiKey
      model: "qwen3.7-plus"
      sendReasoningContent: true
```

**Update "Trigger Conditions"** (lines 5–25): the "no unresolved tool calls" wording is now
enforced by real id matching (previously a stub). Add one line under each mode:
"Resolution is matched on tool-call ids (`tool_calls[].id` vs `toolCallId`)."

**Update "Error Handling"** (lines 106–111) to mention: conversion errors park the chat
with `ai_completions:error` and nothing is sent.

### 2. `plugins/README.md`

**Extend the "Tool Call Tags" section** (starts line 38) — or add a sibling "Tool Messages"
section right after it — with how a plugin creates a tool result message:

```markdown
## Tool Messages

A plugin that executes tools posts results as regular messages:

- `addMessage` with `role: "tool"` **and** `toolCallId` (the id of the assistant
  tool call being answered). The server rejects `role: "tool"` without `toolCallId`.
- The id is stored immutably and reaches the AI provider in `tool.tool_call_id` on the
  next request.
- `addQueueMessage` accepts the same optional `toolCallId`; the id survives promotion
  into the conversation.

Per-call `tags` on assistant `tool_calls` (see "Tool Call Tags") are RHD-internal
orchestration metadata: they are **never** sent to the AI provider.
```

**Update the "Plugin Responsibilities" section** (line 46) if it lists what the
ai_completions plugin does, to reference the fidelity rules above.

## Implementation Notes

1. **Docs must match shipped behavior, not intentions** — write this phase last, after
   phases 1–6, and re-check every claim against the code (especially the error-message
   role-vs-tag fix, which was a documented-but-wrong claim before this milestone).
2. Keep the plugin README's existing structure/headings; insert new sections where they
   read naturally (the exact anchors above).
3. The milestone plan's own "Non-goals" line about corrupted-chat repair belongs in the
   plugin README as an explicit "not implemented yet" note so operators don't expect
   auto-recovery.

## Validation

- `mise run check-cargo` / `mise run test-cargo` (unchanged, sanity).
- Manual review: every statement in the two READMEs is true of the code as merged.
