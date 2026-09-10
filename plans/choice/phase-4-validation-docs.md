# Phase 4: Validation & Knowledge Base Updates

> Parent plan: [`plans/rhd-plugin-choice-plan.md`](../rhd-plugin-choice-plan.md)

## Overview

Prove the feature end-to-end (automated checks + a manual walkthrough with the
real server, plugins, and frontend), then record the new knowledge in the memory
base and root README. No functional code changes expected; fix anything the
walkthrough surfaces.

**Scope in:** test/check commands, manual E2E script, memory doc edits, root
README plugin list, plugin README final pass.
**Scope out:** new features.

**Dependencies:** Phases 1–3 complete.

## Automated Validation

```bash
# Backend (repo root)
mise run check            # cargo check across the workspace (incl. rhd_plugin_choice)
cargo test -p rhd_plugin_choice

# Frontend
cd frontend && npm run check && npm test
```

All green, no new warnings.

## Manual End-to-End Walkthrough

Runbook follows the "Manual Testing Workflow" section of `README.md` (lines
86–113), extended with the choice plugin and frontend. Prereqs: an AI provider
config for `rhd_plugin_ai_completions` (see `memory/configuration.md`).

1. **Start the server:** `cargo run --bin rhd_chat_server`
2. **Start plugins:**
   ```bash
   cargo run -p rhd_plugin_ai_completions -- --server-url ws://127.0.0.1:8080/ <ai-config args>
   cargo run -p rhd_plugin_choice -- --server-url ws://127.0.0.1:8080/
   ```
   Expect `choice` in `rhd plugins list` (or the frontend Plugins tab) and
   `tool registered` in the plugin log for each existing chat.
3. **Start the frontend:** `cd frontend && npm run dev`
4. **Create a chat** in the UI → open the **Tools tab**: `rhd_choice` listed
   (owner plugin `choice`). Also verify a chat created **before** the plugin
   started got the tool (startup reconciliation).
5. **Prompt the model** to use the tool, e.g. "Ask me which of two deployment
   strategies to use via rhd_choice."
6. **Verify the card:** question text renders; one button per option; manual
   text input present. During streaming only the generic collapsible shows.
7. **Click an option** → card flips to "Answered: <option text>"; a `role: tool`
   message with `toolCallId` appears in Messages tab; the assistant continues
   (ToolLoopContinuation) and its next reply references the chosen option.
8. **Second question → type a custom answer** in the input and submit → same
   resolved behavior, content = typed text; assistant continues.
9. **Negative checks:** reload the page after answering — resolved state is
   derived from persisted messages and still renders correctly; an assistant
   message with a malformed `rhd_choice` arguments blob falls back to the
   generic tool-call view (can be simulated by editing the tool-call JSON via
   server API if needed).

## Files to Modify (documentation)

### 1. `memory/features/plugins.md`

Add a **`### Choice Plugin`** subsection under "Example Plugins", after
"### MCP Plugin" (before "### Future Plugin Ideas", ~line 296). Product view
only (no schemas/paths): what it does (assistant asks the user to choose),
how the user answers (buttons / free text), that answering is a human UI
action, and that an unanswered choice intentionally pauses the assistant's
tool loop.

### 2. `memory/file-structure.md`

Extend the tree (follow existing entry style):

```text
plugins/rhd_plugin_choice/src/
└── plugin.rs             # Registers rhd_choice tool per chat (no tool-call handling)
templates/mcp_internal/rhd_choice/
└── tool_definition.json  # rhd_choice tool schema (question + options)
frontend/src/lib/components/
├── ChoicePrompt.svelte   # Question card: option buttons + manual answer input
```

and a note on `chatApiImpl.ts` / `ChatStore.ts` gaining `addMessage` /
`addToolResult` if that file lists API-layer entries.

### 3. `README.md` (root)

- "Packages"/"Plugins" area: add `rhd_plugin_choice` one-liner next to the
  `rhd_plugin_ai_completions` entries (lines ~125, ~131): provides the
  `rhd_choice` tool; the frontend renders choices and answers on the user's
  behalf.

### 4. `plugins/rhd_plugin_choice/README.md`

Final consistency pass against the implemented behavior (created in Phase 1):
CLI usage, tool contract table, "frontend answers, plugin does not" note,
unanswered-choice-blocks-loop behavior.

### 5. `plans/rhd-plugin-choice-plan.md`

When the feature is merged, run the `plans-archive` skill (archive the plan,
consolidate knowledge into `memory/features/plugins.md`).

## Success Criteria (feature-level, from the parent plan)

1. `rhd_plugin_choice` builds and runs; `rhd_choice` is registered in every
   existing chat at startup and in every newly created chat (visible in the
   Tools tab).
2. An assistant `rhd_choice` call renders the question with one button per
   option plus a free-text input.
3. Clicking an option posts a tool message whose content is exactly that
   option's text; submitting the input posts the typed message; both carry the
   correct `toolCallId`.
4. After answering, the card shows the resolved state and the assistant
   continues the turn using the answer.
5. `mise run check`, `cargo test -p rhd_plugin_choice`, frontend
   `npm run check` + `npm test` all pass; memory docs and READMEs updated.

## Dependencies

- Depends on: Phases 1, 2, 3.
- Blocks: nothing (final phase).
