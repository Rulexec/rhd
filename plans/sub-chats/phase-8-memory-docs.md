# Phase 8: Memory Base and Documentation Update

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md)

## Overview

Bring the knowledge base in line with shipped behavior so future work builds on facts: document the `paused` platform tag, the sub-chat plugin (product view + implementation hooks), the `rhd start` wiring example, and the root README plugin roster. Content must describe **final Phase 7-validated behavior** — if any assertion drifted from the grand-plan wording during implementation, the memory files get the implemented truth.

**Scope in:** `memory/` files below, root `README.md`.
**Scope out:** any code changes; plugin `README.md` (created in Phase 2, only typo-level touch-ups here if needed).

**Dependencies:** Phase 5 behavior frozen; ideally after Phase 7 green (the plan explicitly allowed bug fixes there — docs follow fixes). Can be drafted in parallel with Phase 7.

## Files to Modify

### 1. `memory/features/plugins.md`

- **"Example Plugins" → new section "Sub Chat Plugin"** (product-view style of the sibling sections): the three tools and their locked contracts (`async` semantics, exact notice wording is implementation-flavored — keep it to "returns immediately with the subchat id"; completion = "subchat idle with a final answer"; direct-child rule; no timeouts; parked-on-error = pending until operator fixes), tag vocabulary with lineage semantics (`root:` inheritance), two-phase activation via `paused`, the zero-persistence recovery model ("unfinished work is reconstructed at startup from unresolved tool calls + link tags").
- **AI Completions section — Chat Tags list:** add `paused` — platform-level pause honored by trigger detection (never triggers; removing the tag resumes normal flow; does not affect in-flight requests).
- **"Plugin State"/conventions sections:** no new mechanisms were added — leave untouched.

### 2. `memory/architecture.md`

Under **rhd_chat_server** or a small new bullet near the tag discussion: one line — "`paused` is a server-visible chat tag with cross-plugin meaning: `rhd_plugin_ai_completions` skips triggering on paused chats (trigger_detection gate)." Keep it implementation-facing (that's what this file is for). No schema/DB notes (none changed).

### 3. `memory/configuration.md`

Section "rhd start — Process Supervisor Config": extend the children example list with the new plugin (binary takes only CLI args, no config file):

```yaml
- name: sub_chat
  cmd: ./target/release/rhd_plugin_sub_chat
  args: ["--server-url", "ws://127.0.0.1:8080/"]
```

(Exact shape: match the surrounding example already in the file when editing — the file documents `ChildConfig { name, cmd, cwd?, args? }`.)

### 4. `README.md` (repo root)

- Plugin roster bullet (the "Project Structure" listing near line 125): add **`rhd_plugin_sub_chat`**: delegated subchat plugin (spawn/status/await) — mirroring the existing entries' one-liner style. The earlier structure list (around line 10) mentions only ai_completions as an example — leave unless the list is exhaustive (check while editing; keep consistent with how choice/mcp are listed).

### 5. `memory/MEMORY.md`

Index row for `features/plugins.md`: extend the "when to read" description with "sub chat plugin (subchat spawning, paused tag)". (The "AI completions plugin (tags, …)" fragment already covers the gate implicitly — only add `paused` if the row lists tag names.)

### 6. `plugins/rhd_plugin_sub_chat/README.md` (review only)

Created in Phase 2; verify it matches implemented behavior (tag list, error policies, "no config file") — fix any drift.

## Tests / Verification

- No code tests. Verification = factual accuracy: cross-read each doc claim against final code (`trigger_detection.rs` gate, `tags.rs` constants, `spawn.rs` tag set, `handler.rs` error texts, `completion.rs` predicate) and spot-check one claim per section.
- Run the `memory-consistency` skill after edits per project convention for memory maintenance.

## Implementation Notes

1. **Product-view discipline:** `features/plugins.md` gets behavior ("what the model/user observes"); `architecture.md` gets mechanism ("which function gates what"). Don't leak function names into the features file; don't duplicate — follow the two-layer pattern stated in `MEMORY.md`.
2. **The `paused` story spans two files on purpose:** features/plugins.md describes it in the AI Completions + Sub Chat sections (behavior); architecture.md carries the one-line cross-plugin fact. That's the established split.
3. **Do not archive the plans** in this phase — `plans-archive` is a separate maintenance step performed once the milestone is fully accepted.

## Dependencies

- **Requires:** Phase 5 (frozen behavior); finalizes after Phase 7 (validated).
- **Blocks:** nothing (terminal phase; grand-plan completion gate includes it).
