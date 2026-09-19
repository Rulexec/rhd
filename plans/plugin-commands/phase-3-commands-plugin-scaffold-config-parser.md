# Phase 3: `rhd_plugin_commands` — Crate Scaffold, Config Model, Command Parser

## Overview

Create the new plugin crate and implement its two pure (I/O-free) building blocks:

1. **Config model** — YAML `commands:` map supporting four shapes (string shorthand, tagged single command, multi-step list), validated at startup with prompt files cached.
2. **Command parser** — given a queued message's content and the registry of command names, extract the leading command invocations and the remaining text.

**In scope:** crate skeleton, `main.rs`, `config.rs`, `parser.rs`, workspace registration, unit tests. `plugin.rs` provides a minimal lifecycle stub (connect/register/keepalive) so the binary runs; the event-handling executor is Phase 4.
**Out of scope:** chat protocol interaction beyond registration (Phase 4), events (Phase 2 produces them; this plugin only compiles against the name constant here).

**Dependencies:** none — independent of Phases 1–2 (the executor in Phase 4 uses their outputs).

## Files to Create/Modify

### 1. `Cargo.toml` (workspace root)

Add `"plugins/rhd_plugin_commands",` to `[workspace] members` after `"plugins/rhd_plugin_choice",`.

### 2. `plugins/rhd_plugin_commands/Cargo.toml`

```toml
[package]
name = "rhd_plugin_commands"
version = "0.1.0"
edition = "2021"

[dependencies]
rhd_chat_client = { path = "../../packages/rhd_chat_client" }
rhd_chat_api = { path = "../../packages/rhd_chat_api" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive"] }
thiserror = "1"

[dev-dependencies]
tempfile = "3"
chrono = "0.4"
```

(Dev-deps grow in Phase 5 when e2e tests land: `rhd_chat_server`, `rhd_db`, `rhd_mock_ai_provider`, `rhd_plugin_ai_completions`.)

### 3. `plugins/rhd_plugin_commands/src/lib.rs`

```rust
pub mod config;
pub mod parser;
pub mod plugin;

/// Custom event handled by this plugin (emitted by rhd_plugin_ai_completions).
pub const PRE_DRAIN_QUEUE_EVENT: &str = "ai_completions:preDrainQueue";

/// Tag prefix added to queued prompt messages for observability. The tag
/// survives the drain into regular messages.
pub fn prompt_tag(name: &str) -> String {
    format!("commands:prompt:{}", name)
}
```

### 4. `plugins/rhd_plugin_commands/src/main.rs`

Mirror `rhd_plugin_system_prompt/src/main.rs` exactly (same structure, same arg style):

```rust
//! Entry point for the slash-commands plugin.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_commands::{config, plugin};

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_commands")]
#[command(about = "Slash-command plugin for RHD queued messages")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "commands")
    #[arg(long, default_value = "commands")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_commands=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Fail fast on bad config / missing prompt files
    let registry = config::load_config(&args.config)?;

    tracing::info!("Starting commands plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Loaded {} commands", registry.len());

    plugin::run_plugin(&args.server_url, &args.plugin_id, registry).await?;
    Ok(())
}
```

### 5. `plugins/rhd_plugin_commands/src/config.rs`

Raw YAML model (four accepted shapes). Note: untagged `PromptPath(String)` must be tried **before** `Spec` so bare strings resolve to the shorthand:

```rust
//! Configuration loading and validation for the commands plugin.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    pub commands: HashMap<String, CommandDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandDef {
    Multi(Vec<CommandStep>),
    Single(CommandStep),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandStep {
    /// Shorthand: a path to a markdown prompt file, inserted as role=user.
    PromptPath(String),
    Spec(CommandSpec),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSpec {
    #[serde(deny_unknown_fields)]
    ChatTags {
        #[serde(default)]
        add: Vec<String>,
        #[serde(default)]
        remove: Vec<String>,
    },
    #[serde(deny_unknown_fields)]
    MessageTags {
        #[serde(default)]
        add: Vec<String>,
        #[serde(default)]
        remove: Vec<String>,
    },
    #[serde(deny_unknown_fields)]
    Prompt {
        /// One of: user (default), system, assistant.
        #[serde(default = "default_prompt_role")]
        role: String,
        /// Path to a markdown file; resolved against the config directory.
        prompt: String,
    },
}

fn default_prompt_role() -> String {
    "user".to_string()
}
```

Resolved model produced after validation + prompt caching (this is what the executor consumes; keep `Debug, Clone, PartialEq, Eq` for tests):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedStep {
    ChatTags { add: Vec<String>, remove: Vec<String> },
    MessageTags { add: Vec<String>, remove: Vec<String> },
    Prompt { name: String, role: String, content: String },
}

#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: HashMap<String, Vec<ResolvedStep>>,
}

impl CommandRegistry {
    pub fn has(&self, name: &str) -> bool { ... }
    pub fn steps(&self, name: &str) -> Option<&[ResolvedStep]> { ... }
    pub fn names(&self) -> HashSet<String> { ... }
    pub fn len(&self) -> usize { ... }
}
```

`load_config(path: &str) -> Result<CommandRegistry, ConfigError>`:

1. Read + `serde_yaml::from_str::<PluginConfig>` (`ConfigError::Parse` / `MainConfigFileRead`, same shape as `rhd_plugin_system_prompt/src/config.rs:34`).
2. Command name check: `fn is_valid_command_name(name: &str) -> bool` — non-empty, every char `is_ascii_alphanumeric() || c == '_'` (equivalent of `^[A-Za-z0-9_]+$` without a regex dep).
3. Per-step resolution:
   - `PromptPath(p)` → `ResolvedStep::Prompt { name: <command name>, role: "user", content: read_prompt(&p)? }`
   - `ChatTags` / `MessageTags` → reject when both `add` and `remove` are empty (`ConfigError::EmptyTags { name }`); reject empty tag strings.
   - `Prompt { role, prompt }` → `role` must be `user`/`system`/`assistant` (`ConfigError::InvalidPromptRole { name, role }` — `tool` would require a `toolCallId`; rejected at startup, not runtime); read file.
4. `read_prompt(rel)`: resolve relative paths against the config file's directory (copy `resolve_path` from `rhd_plugin_system_prompt/src/config.rs:75` verbatim), read to string verbatim — markdown files, no YAML parsing of content. Missing/unreadable → `ConfigError::PromptFileRead { name, path, details }` (fail fast, same convention as system_prompt plugin).

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read main config file '{path}': {details}")]
    MainConfigFileRead { path: String, details: String },
    #[error("failed to parse config: {0}")]
    Parse(String),
    #[error("invalid command name '{0}': only [A-Za-z0-9_] allowed")]
    InvalidCommandName(String),
    #[error("failed to read prompt file for command '{name}' at '{path}': {details}")]
    PromptFileRead { name: String, path: String, details: String },
    #[error("command '{name}' has prompt role '{role}' (allowed: user, system, assistant)")]
    InvalidPromptRole { name: String, role: String },
    #[error("command '{name}' has neither add nor remove tags")]
    EmptyTags { name: String },
}
```

### 6. `plugins/rhd_plugin_commands/src/parser.rs`

Pure function, byte-cursor scan. API contract:

```rust
/// Result of parsing leading commands out of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommands {
    /// Registered command names, in occurrence order (repeats allowed).
    pub invocations: Vec<String>,
    /// Message content after the last recognized command, whitespace-trimmed.
    /// When parsing stopped at an unknown "/token", the remainder starts with
    /// that token VERBATIM (it and everything after it is plain text).
    pub remainder: String,
}

/// Returns None when the message carries no recognized command
/// (does not start with '/', or first token is unknown) — leave it untouched.
/// Returns Some(..) as soon as at least one command matched.
pub fn parse(content: &str, registry: &HashSet<String>) -> Option<ParsedCommands>
```

Algorithm:

```rust
pub fn parse(content: &str, registry: &HashSet<String>) -> Option<ParsedCommands> {
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0usize;
    let mut invocations: Vec<String> = Vec::new();

    loop {
        // 1. skip any whitespace before the next token
        while i < chars.len() && chars[i].is_whitespace() { i += 1; }
        if i >= chars.len() { break; }                 // consumed everything
        if chars[i] != '/' { break; }                  // plain-text remainder starts here

        // 2. read the command name: [A-Za-z0-9_]+
        let mut j = i + 1;
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
            j += 1;
        }
        let name: String = chars[i + 1..j].iter().collect();

        // 3. empty or unknown name -> stop; remainder is the raw text from '/'
        if name.is_empty() || !registry.contains(&name) {
            if invocations.is_empty() {
                return None;                           // untouched message
            }
            let remainder = chars[i..].iter().collect::<String>();
            return Some(ParsedCommands { invocations, remainder });
        }

        invocations.push(name);
        i = j;                                         // continue right after the name
    }

    if invocations.is_empty() {
        return None;
    }
    let remainder: String = chars[i..].iter().collect();
    Some(ParsedCommands { invocations, remainder: remainder.trim().to_string() })
}
```

Semantics encoded (also documented as doc-comment examples on `parse`):

| input (registry has `a`, `b`) | result |
|---|---|
| `hello` | `None` |
| `/a hello` | `["a"]`, `"hello"` |
| `"  \n /a"` | `["a"]`, `""` |
| `/a/b x` | `["a","b"]`, `"x"` (adjacent commands) |
| `/a /b` | `["a","b"]`, `""` |
| `/a /unknown x` | `["a"]`, `"/unknown x"` (verbatim, parsing stopped) |
| `/unknown /a x` | `None` (first token unknown → message untouched; `/a` is NOT executed) |
| `/a/` | `["a"]`, `"/"` (empty name after trailing slash = unknown token) |
| `/` | `None` |
| `/a b/c` | `["a"]`, `"b/c"` |

**Design notes to include in the module docs:**
- Whitespace between commands is optional; the name terminates at any non-`[A-Za-z0-9_]` char (so `-`, `.`, etc. end the name and belong to the remainder).
- Unknown-token stop keeps the raw text from the '/' onward **untrimmed** at that point so re-emergent content reads naturally, but the final returned remainder is `trim()`-ed.
- The parser never validates step counts or side effects — occurrence order is preserved for the executor to expand via the registry.

### 7. `plugins/rhd_plugin_commands/src/plugin.rs` (minimal stub — executor comes in Phase 4)

Follow the first ~80 lines of `rhd_plugin_system_prompt/src/plugin.rs` structure:

```rust
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    registry: config::CommandRegistry,
) -> Result<(), PluginError> {
    let client = Arc::new(
        ChatClient::connect_with_retry(server_url).await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    client.register_plugin(RegisterPluginParams { plugin_id: plugin_id.to_string() })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    // Phase 4 adds: getPendingAcks processing, on_custom_event subscription, executor.
    loop {
        tokio::time.sleep(std::time::Duration::from_secs(60)).await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
}
```

## Tests

### `config.rs` unit tests (`#[cfg(test)] mod tests`, tempfile pattern from `rhd_plugin_system_prompt/src/config.rs:103`)

Write config + prompt `.md` files into a `tempfile::TempDir`. Happy-path — one test asserting the full resolved registry against the canonical example:

```yaml
commands:
  tags_example:
    type: chat_tags
    add: [mcp:common]
    remove: [pause]
  prompt_example: ./commands/prompt.md
  system_prompt_example:
    type: prompt
    role: system
    prompt: ./systemPrompts/warhammer.md
  multi_example:
    - type: message_tags
      add: [some_tag]
    - ./commands/prompt.md
    - ./commands/another_prompt.md
```

Error cases (each asserts the specific `ConfigError` variant):
1. unknown top-level field → `Parse` (deny_unknown_fields),
2. `type: chat_tags` with neither `add` nor `remove` → `EmptyTags`,
3. `role: tool` → `InvalidPromptRole`,
4. missing prompt file for shorthand/tagged/multi step → `PromptFileRead` (message includes resolved path),
5. command key `bad-name` → `InvalidCommandName`,
6. spec object without `type` → `Parse` error (untagged fallthrough: it is not a string, and the tagged enum rejects it),
7. relative-path resolution: run from a config file in a subdir; prompt path `./prompts/x.md` resolves next to the config (absolute path also accepted as-is).

### `parser.rs` unit tests

One `#[test]` per table row above (name them `test_parse_<case>`), plus:
- `test_parse_repeated_command` — `/a /a x` → `["a","a"]`, `"x"`.
- `test_parse_preserves_remainder_internals` — `/a  one   two ` → remainder `"one   two"` (interior whitespace intact, edges trimmed).
- `test_parse_multibyte_content` — `/a привет` → remainder `привет` (char-based cursor, not byte slicing).

## Implementation Notes

1. **Untagged enum ordering is load-bearing**: `CommandDef::Multi` before `Single`, and `CommandStep::PromptPath` before `Spec`; serde tries variants in declaration order. Unit-test #6 guards regressions here.
2. **Prompt content is cached at startup** (like `CachedPrompt` in system_prompt); edits to `.md` files require a plugin restart. Consistent with project convention; document in README (Phase 4).
3. **No `regex` dependency** — hand-rolled char-class checks keep deps aligned with existing plugins.
4. **Registry key set is the parser's only input** — deliberately decoupled from step resolution so the parser stays trivially testable.
5. **Command names are globally unique by construction** (HashMap keys); a name that equals another with different case is a distinct command (`Foo` vs `foo`) — parsing is case-sensitive; document in README.
6. **File size**: keep `config.rs` and `parser.rs` well under the 400/500-line limits by keeping tests in the same file's `mod tests` and deferring all chat-protocol code to `plugin.rs`/`executor.rs` (Phase 4).

## Dependencies

- Depends on: nothing.
- Blocks: Phase 4 (executor consumes `CommandRegistry` and `parser::parse`; uses `PRE_DRAIN_QUEUE_EVENT`).
- Parallel with: Phases 1 and 2.
