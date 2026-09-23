# Phase 2: `rhd_plugin_sub_chat` Crate Scaffold, Lifecycle, Tool Registration

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-8, AD-9, AD-10)

## Overview

Create a compiling, running `rhd_plugin_sub_chat` plugin that registers with the chat server, drains/acknowledges custom events, watches all chats via `ChatMonitor`, and registers the three sub-chat tools in **every** chat (choice-plugin pattern). After this phase the tools are visible in the Tools tab of every chat, but tool calls are **not answered yet** (Phase 3+). The plugin's tag vocabulary module (`tags.rs`) and embedded tool-definition templates are built here as the shared foundation of all later phases.

**Scope in:** workspace membership, crate scaffold, CLI args, lifecycle skeleton, per-chat tool registration, `tags.rs`, `templates.rs`, three `tool_definition.json` files, plugin README, tags unit tests.
**Scope out:** `on_tool_call` subscription/handlers (Phase 3/5), watcher (Phase 4), recovery (Phase 6). Intermediate state note: between this phase and Phase 3, a model calling `rhd_sub_chat` would park its loop (unanswered call) — acceptable mid-milestone; the plugin is not shipped half-way.

**Dependencies:** none. Parallel with Phase 1.

## Wire Contract (fixed — all later phases rely on these exact names)

| Item | Value |
|---|---|
| Crate / binary | `plugins/rhd_plugin_sub_chat`, bin `rhd_plugin_sub_chat` |
| Plugin id (CLI default) | `rhd_plugin_sub_chat` |
| Tools | `rhd_sub_chat`, `rhd_sub_chat_status`, `rhd_sub_chat_await` |
| Chat tags | `paused`; `parent:<chatId>`; `root:<chatId>`; `sub_chat:call:<toolCallId>` |
| Modules | `main.rs` `lib.rs` `plugin.rs` `tags.rs` `templates.rs` (later: `reply.rs` `spawn.rs` `handler.rs` `completion.rs` `watcher.rs` `recovery.rs`) |
| Subchat title format | `sub · <first 40 chars of first user message>` |

## Files to Create/Modify

### 1. `Cargo.toml` (workspace root)

**Modify:** add to `members`: `"plugins/rhd_plugin_sub_chat"` (after `plugins/rhd_plugin_commands`).

### 2. `plugins/rhd_plugin_sub_chat/Cargo.toml` (new)

Mirror `plugins/rhd_plugin_choice/Cargo.toml` exactly in shape:

```toml
[package]
name = "rhd_plugin_sub_chat"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "rhd_plugin_sub_chat"
path = "src/main.rs"

[dependencies]
rhd_chat_api = { path = "../../packages/rhd_chat_api" }
rhd_chat_client = { path = "../../packages/rhd_chat_client" }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
include_dir = "0.7"
clap = { workspace = true, features = ["derive"] }

[dev-dependencies]
rhd_chat_server = { path = "../../packages/rhd_chat_server" }
tempfile = "3"
```

(`chrono` added later only if a phase needs it.)

### 3. `plugins/rhd_plugin_sub_chat/src/main.rs` (new)

Copy `rhd_plugin_choice/src/main.rs` structure verbatim with renames:

- `Args { #[arg(long)] server_url: String, #[arg(long, default_value = "rhd_plugin_sub_chat")] plugin_id: String }` — **no `--config`** (the plugin has no settings and no credentials).
- tracing init with directive `rhd_plugin_sub_chat=info`.
- `plugin::run_plugin(&args.server_url, &args.plugin_id).await?`.

### 4. `plugins/rhd_plugin_sub_chat/src/lib.rs` (new)

```rust
pub mod plugin;
pub mod tags;
pub mod templates;
```

(Each later phase appends its module here: `reply`, `spawn`, `handler` in Phase 3; `completion`, `watcher` in Phase 4; `recovery` in Phase 6.)

### 5. `plugins/rhd_plugin_sub_chat/src/tags.rs` (new)

Single source of truth for the tag vocabulary. Full API (pure, no client deps):

```rust
//! Chat tag vocabulary for the sub-chat plugin.

/// Platform-level pause tag honored by ai_completions (exact match, bare name).
pub const PAUSED_TAG: &str = "paused";

pub const PARENT_PREFIX: &str = "parent:";
pub const ROOT_PREFIX: &str = "root:";
pub const CALL_PREFIX: &str = "sub_chat:call:";

/// Tool-call → subchat link tag (set atomically at createChat).
pub fn call_tag(tool_call_id: &str) -> String { format!("{CALL_PREFIX}{tool_call_id}") }

/// Parses `sub_chat:call:<id>` → Some(id).
pub fn parse_call_tag(tag: &str) -> Option<&str>;

pub fn parent_tag(chat_id: i64) -> String { format!("{PARENT_PREFIX}{chat_id}") }
pub fn root_tag(chat_id: i64) -> String { format!("{ROOT_PREFIX}{chat_id}"); }

/// The topmost root id declared in a tag set, if any.
pub fn root_of(tags: &[String]) -> Option<i64>;   // parses `root:<int>`, numeric-only

/// True if the tagged chat is a direct child of `parent_chat_id`
/// (carries exactly `parent:<parent_chat_id>`).
pub fn is_direct_child(tags: &[String], parent_chat_id: i64) -> bool;

pub fn has_paused_tag(tags: &[String]) -> bool;

/// Validate user-supplied spawn tags: reject reserved shapes —
/// any of `parent:` / `root:` / `sub_chat:` prefixes, exact `paused`,
/// empty string, or embedded whitespace. Returns a joined error message.
pub fn validate_user_tags(tags: &[String]) -> Result<(), String>;
```

Implementation notes: parse with `strip_prefix` + `str::parse::<i64>()` (malformed → not ours). All matches are **exact** (no `starts_with` except the reserved check in `validate_user_tags`).

### 6. `plugins/rhd_plugin_sub_chat/src/templates.rs` (new)

Copy `rhd_plugin_choice/src/templates.rs` shape; `include_dir!("$CARGO_MANIFEST_DIR/../../templates")`; three constants and accessors:

```rust
pub const SPAWN_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat/tool_definition.json";
pub const STATUS_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat_status/tool_definition.json";
pub const AWAIT_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat_await/tool_definition.json";

pub struct Templates { spawn: String, status: String, await_tool: String }
impl Templates {
    pub fn load() -> Result<Self, TemplateError>;
    pub fn spawn_definition(&self) -> &str;
    pub fn status_definition(&self) -> &str;
    pub fn await_definition(&self) -> &str;
}
```

(`await` is a Rust keyword — the field/method is `await_tool` / `await_definition` as above; JSON files keep the real tool names.)

### 7. Tool definition JSONs (new, under `templates/mcp_internal/`)

`rhd_sub_chat/tool_definition.json` — locked schema:

```json
{
  "type": "function",
  "function": {
    "name": "rhd_sub_chat",
    "description": "Delegate a self-contained task to a fresh subchat: a new chat that runs with its own history and tools and finishes with a final assistant answer. Provide the subchat conversation seed as ordered messages with roles 'system' (instructions for the subchat) and 'user' (the task). Tags are applied to the subchat BEFORE its messages are queued, so prompt/tool-gating tags take effect on its very first request. In default (sync) mode this call returns only when the subchat has finished, with its final assistant message as the result. Set async=true to start it and return immediately with the subchat id; inspect it later with rhd_sub_chat_status or rhd_sub_chat_await. A subchat that errored stays pending until an operator fixes it.",
    "parameters": {
      "type": "object",
      "properties": {
        "messages": {
          "type": "array",
          "description": "Ordered seed messages for the subchat. Roles restricted to system and user.",
          "items": {
            "type": "object",
            "properties": {
              "role": { "type": "string", "enum": ["system", "user"] },
              "content": { "type": "string" }
            },
            "required": ["role", "content"]
          }
        },
        "tags": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Extra chat tags for the subchat (e.g. systemPrompt:<name>, worktree:<id>, mcp:<tag>). Reserved tags (paused, parent:, root:, sub_chat:) are rejected."
        },
        "async": {
          "type": "boolean",
          "description": "If true, return immediately after starting the subchat; if false or omitted, return the subchat's final answer when it completes."
        }
      },
      "required": ["messages"]
    }
  }
}
```

`rhd_sub_chat_status/tool_definition.json`:

```json
{
  "type": "function",
  "function": {
    "name": "rhd_sub_chat_status",
    "description": "Check a subchat you spawned from this chat. Returns exactly 'pending' (still running, queued, or parked on an error) or 'completed' (idle with a final assistant message). Only direct children of the current chat can be queried.",
    "parameters": {
      "type": "object",
      "properties": {
        "chatId": { "type": "integer", "description": "Id of the subchat, as reported by rhd_sub_chat" }
      },
      "required": ["chatId"]
    }
  }
}
```

`rhd_sub_chat_await/tool_definition.json`: same shape/description tone, name `rhd_sub_chat_await`, description stating it **holds the tool call until the subchat finishes** and then returns the last assistant message content verbatim; only direct children; no timeout.

### 8. `plugins/rhd_plugin_sub_chat/src/plugin.rs` (new)

Adapt `rhd_plugin_choice/src/plugin.rs` (same skeleton, this is the reference implementation — keep its comment style):

```rust
pub async fn run_plugin(server_url: &str, plugin_id: &str) -> Result<(), PluginError> {
    // 1. Templates::load()
    // 2. ChatClient::connect_with_retry
    // 3. register_plugin(RegisterPluginParams { plugin_id })
    // 4. get_pending_acks drain + ack loop
    // 5. on_custom_event → ack every event immediately (memory/development.md
    //    "Every Plugin Must Acknowledge Live Custom Events" — mandatory;
    //    unacked preRequest would park this plugin's own subchats)
    // 6. create_chat_monitor + subscribe_to_all_chats
    // 7. on_chat_state_change: per-chat addTools with all three definitions,
    //    once per chat via initialized_chats: Arc<RwLock<HashSet<i64>>>;
    //    NEVER await inline — tokio::spawn the body (copy choice pattern incl.
    //    the "retry on next state change" behavior on addTools failure)
    // 8. keep-alive loop
}
```

Parse each embedded JSON into `rhd_chat_api::ToolDefinition` with `serde_json::from_str` (as choice does) and register all three in one `AddToolsParams { chat_id, tools: vec![spawn, status, await] }` call.

`PluginError` enum: same variants as choice's (`Template`, `Connection`, `Registration`, `PendingAcks`, `MonitorCreate`, `Subscription`) — `thiserror`, `String` payloads.

**Not in this phase:** `on_tool_call` subscription — do not add it; Phase 3 owns it.

### 9. `plugins/rhd_plugin_sub_chat/README.md` (new)

Follow the `plugins/README.md` template: Overview (delegation to subchats), Trigger Conditions (tool calls per chat), Events Emitted (none), Tags Added (`parent:`, `root:`, `sub_chat:call:`, `paused` on spawn — mark that later phases activate it; `paused` consumption note: honored by `ai_completions` since Phase 1), Configuration Format (none — CLI only), Dependencies (`rhd_chat_client`, `ai_completions` plugin for processing subchats), Error Handling summary.

## Tests

- **`tags.rs` inline unit tests:** roundtrip builders/parsers; `parse_call_tag` rejects `sub_chat:call_` near-miss; `root_of` ignores `root:abc` (non-numeric); `is_direct_child` exact id (child of 12 is not child of 1); `validate_user_tags` rejects `paused`, `parent:5`, `root:9`, `sub_chat:call:x`, empty, whitespace-containing; accepts `systemPrompt:plan`, `worktree:wt1`, `mcp:common`.
- **`templates.rs` inline tests:** `Templates::load()` succeeds; each string parses as `rhd_chat_api::ToolDefinition` with the expected `function.name`.
- **`plugins/rhd_plugin_sub_chat/tests/custom_event_ack_test.rs`** — copy the guard-test approach of `plugins/rhd_plugin_choice/tests/custom_event_ack_test.rs`: start chat server, run `plugin::run_plugin` in a tokio task, connect a second client, send `ai_completions:preRequest` custom event, assert ack arrives (no 30 s timeout).
- **`tests/integration_test.rs`** — chat server + plugin; create chat; poll `get_tools` until all three names appear; create a second chat, assert both get all three tools (per-chat registration on `chatCreated`).

## Implementation Notes

1. **Registration must be per (chat) once**, guarded by `initialized_chats`; `addTools` is additive server-side but re-sending on every state change is wasteful and noisy (choice already models this).
2. **Subscribed chats include plugin-created ones for free:** `subscribe_to_all_chats` + `chatCreated` handling means subchats spawned in Phase 3 will have the tools registered before/at activation — recursion works without extra code (AD-9).
3. **No config file on purpose** — everything is CLI. If future settings appear, follow `rhd_plugin_system_prompt/src/config.rs`.
4. **File size:** keep each module well under the project's 400/500-line split threshold; `handler.rs`/`spawn.rs` phases should split by concern, not pile into `plugin.rs`.

## Dependencies

- **None** (parallel with Phase 1; only dev-dependency is the existing server crate).
- **Must complete before** Phases 3–6 (module skeleton, `tags.rs`, `templates.rs`).
