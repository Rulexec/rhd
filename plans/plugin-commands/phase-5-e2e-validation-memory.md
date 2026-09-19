# Phase 5: End-to-End Tests, Validation, Memory & Docs Consolidation

## Overview

Prove the whole feature through a real chat server + real AI completions plugin + mock provider, run the project checks clean, and update the knowledge base. This phase owns everything cross-plugin: the commands-plugin e2e suite, final validation, and memory documentation.

**In scope:** `plugins/rhd_plugin_commands/tests/commands_e2e_test.rs`, dev-dependency additions, `mise run check-cargo` / `mise run test-cargo` clean, memory file updates.
**Out of scope:** any behavior change — if e2e exposes a bug, fix it in the owning crate (Phases 1–4) and re-run.

**Dependencies:** ALL previous phases (1: positional insert; 2: `preDrainQueue` emission; 3: config/parser; 4: executor/README).

## Files to Create/Modify

### 1. `plugins/rhd_plugin_commands/Cargo.toml`

Extend `[dev-dependencies]`:

```toml
rhd_chat_server = { path = "../../packages/rhd_chat_server" }
rhd_db = { path = "../../packages/rhd_db" }
rhd_mock_ai_provider = { path = "../../packages/rhd_mock_ai_provider" }
rhd_plugin_ai_completions = { path = "../rhd_plugin_ai_completions" }
tokio = { version = "1", features = ["full", "rt-multi-thread"] }
```

### 2. `plugins/rhd_plugin_commands/tests/commands_e2e_test.rs` (new)

Environment setup copies `TestEnv` from `plugins/rhd_plugin_ai_completions/tests/integration_test.rs:27–105` verbatim in structure (ephemeral-port chat server on `:memory:`, `SimpleListener` mock AI, credentials/config tempfiles), extended with a commands-plugin config:

```rust
struct TestEnv {
    chat_server_port: u16,
    _mock_ai: MockAiProvider,
    ai_config: rhd_plugin_ai_completions::config::PluginConfig,
    commands_config: rhd_plugin_commands::config::CommandRegistry,
    _tmp: TempDir, // holds ai config/creds, commands config.yaml, prompt .md files
}
```

`TestEnv::new()` additionally writes under the temp dir:

```
commands-config.yaml        # the canonical 4-command example (Phase 3 config test YAML)
commands/prompt.md          -> "PROMPT_ONE"
commands/another_prompt.md  -> "PROMPT_TWO"
systemPrompts/warhammer.md  -> "SYSTEM_PROMPT"
```

and loads it via `rhd_plugin_commands::config::load_config`.

Both plugins start in background tasks:

```rust
let ai_handle = tokio::spawn({
    let url = env.chat_server_url();
    let config = env.ai_config.clone();
    async move { rhd_plugin_ai_completions::plugin::run_plugin(&url, "ai_completions", config).await }
});
let cmd_handle = tokio::spawn({
    let url = env.chat_server_url();
    let registry = env.commands_config.clone();
    async move { rhd_plugin_commands::plugin::run_plugin(&url, "commands", registry).await }
});
sleep(Duration::from_millis(500)).await; // both registered
```

Shared helpers: `queue_text(&client, chat_id, content) -> i64`, `get_history(&client, chat_id) -> Vec<Message>`, `get_chat_tags(&client, chat_id) -> Vec<String>`, `wait_until(fn() -> bool, Duration)` (poll with timeout — prefer over fixed sleeps for the assertion targets).

Mock listener is primed with enough canned replies (`listener.push_text(...)` per expected AI request; the completion turn may continue with tool loops in other suites — not here).

## E2E Scenarios (one `#[tokio::test]` each, wrapped in `timeout(Duration::from_secs(20), ...)`)

**1. `test_command_with_trailing_text`** — queue `"/tags_example /prompt_example hello"`.
After the assistant reply lands, assert history is exactly:
- `[0]` user, content `"PROMPT_ONE"`, tags contain `"commands:prompt:prompt_example"`;
- `[1]` user, content `"hello"` (commands stripped, tag-free);
- `[2]` assistant `"Test response"`;
- chat tags contain `"mcp:common"`, do NOT contain `"pause"`; queue is empty.

**2. `test_command_only_message_is_removed`** — queue `"/tags_example /prompt_example"` (nothing but commands).
Assert: no history message with content `"/tags_example /prompt_example"`; `"PROMPT_ONE"` user message present at its position; chat tags as in #1; assistant replied (queue had ≥1 message, drain happened, request built from the remaining history).

**3. `test_adjacent_commands_without_spaces`** — queue `"/tags_example/prompt_example hi"`.
Assert same shape as #1 with remainder `"hi"`.

**4. `test_multi_command_expands_in_order`** — queue `"/multi_example tail"`.
Assert history: `"PROMPT_ONE"`, `"PROMPT_TWO"` (config order!), then `"tail"`; the middle message (`"tail"`) carries tag `"some_tag"`; queue drained.

**5. `test_two_queued_command_messages_keep_interleaved_order`** — queue `"/prompt_example one"`, then (immediately after, before the trigger fires — same tick) `"/prompt_example two"`. If racing the trigger proves flaky, first drive chat into a tool-loop (reuse `tool_call_e2e_test.rs` mechanics) and queue both while `ai_completions:running`, then resolve the tool.
Assert final user-turn order: `PROMPT_ONE`, `one`, `PROMPT_ONE`, `two` — this is the test that justifies the positional-insert API choice over append.

**6. `test_unknown_command_passes_through`** — queue `"/notacommand hello"` and `"/tags_example /notacommand x"`.
Assert: first message drains verbatim; second drains as `"/notacommand x"` (recognized command executed: chat tag `mcp:common` present, remainder verbatim from the unknown token).

**7. `test_non_user_queue_messages_untouched`** — queue an assistant-role message `"/tags_example hidden"` (plus any user message to trigger the drain).
Assert: it drains verbatim with its content intact and chat tags unchanged by that message.

**8. `test_no_commands_plugin_no_behavior_change`** — start ONLY ai_completions; queue `"/tags_example x"`; assert normal drain (`x`... actually content preserved verbatim `"/tags_example x"` as a plain user message), assistant replies — regression guard for the unhandled-event path.

## Validation

1. `mise run check-cargo` — zero warnings preferred; if clippy-level issues appear in touched crates, clean them (project skill `fix-checks` describes the loop). Run it only after the codebase is otherwise stable.
2. `mise run test-cargo` — full suite green, including all pre-existing ai_completions / system_prompt / todo_list / mcp / choice tests (the new `preDrainQueue` wait must be invisible to them: no other plugins registered → instant ack completion).
3. Confirm file-size limits respected (`mise run check-large-files`): `ai_request.rs` was the largest touched file; split further if it crosses the limit.
4. Manual smoke with the CLI per Phase 4 README checklist (optional but recommended before committing).
5. Commits (per project convention, made by the implementing role): one per phase, lowercase, e.g. `add positional queue insertion to addQueueMessage`, `emit ai_completions:preDrainQueue before queue drain`, `add rhd_plugin_commands with preDrainQueue executor`, `commands plugin e2e tests and memory docs`.

## Memory & Documentation Updates

Follow `memory/MEMORY.md` conventions (product-view in `features/`, implementation in top-level files):

1. **`memory/MEMORY.md`** — add to the plugins tree:
   `│   ├── rhd_plugin_commands/        # Queued-message slash-commands plugin`
2. **`memory/features/plugins.md`**:
   - **AI Completions Plugin section** — under "Queued Message Processing" / "Pre-Request Coordination": add the `ai_completions:preDrainQueue` bullet (fires only on queuedMessages trigger, after preRequest acks, before promotion; plugins must ack; timeout parks chat).
   - **New "### Commands Plugin" section** (product view, after "MCP Plugin"/near "System Prompt Plugin"): what slash-commands do for the user, config shapes, message syntax rules (leading position, whitespace, adjacent forms), unknown-command verbatim passthrough, empty-message removal, `commands:prompt:<name>` message tag, prompt caching at startup, always-ack policy, and the crash-window caveat.
3. **`memory/configuration.md`** — one paragraph for `plugins/rhd_plugin_commands/src/config.rs`: `PluginConfig { commands: map name → string-path | tagged spec | list }`, strict validation, path resolution against the config dir, allowed prompt roles.
4. **`memory/protocols.md`** — extend the Queue methods line: "`addQueueMessage` (optional `beforeMessageId` for positional insertion; queue ordered by internal position)".
5. Plugin READMEs are already in-tree (Phases 2 & 4); cross-check `plugins/rhd_plugin_ai_completions/README.md` "Coordination" example section (~line 217) mentions both events.

## Implementation Notes

1. **Determinism:** use `wait_until` on observable state (history length/tags) instead of long fixed sleeps; the existing suites' 500 ms registration delay is the only acceptable fixed wait.
2. **Scenario 5 race:** queueing two messages before the first trigger is genuinely racy (ChatMonitor may fire between adds). The tool-loop parking technique is deterministic; budget implementation time for it — it doubles as proof the queue survives the running window with correct order.
3. **Why no frontend test:** queue position is server-internal; the visible end state (regular messages order) is covered here.
4. **Memory hygiene rule** (from the memory-consistency skill): keep each updated file's added text proportional — describe behavior, not code paths, in `features/`.

## Success Criteria (feature-wide, evaluated at the end of this phase)

1. All milestone-plan scenarios verified by the e2e suite above.
2. `mise run check-cargo && mise run test-cargo` clean.
3. Every touched plugin has an updated README; memory index entries added; plan archivable via the `plans-archive` skill.

## Dependencies

- Depends on: Phases 1–4 (all).
- Blocks: nothing; final validation gate before the milestone plan can be archived.
