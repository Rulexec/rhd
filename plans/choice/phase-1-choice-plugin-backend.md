# Phase 1: Backend — `rhd_plugin_choice` Crate (Tool Registration Only)

> Parent plan: [`plans/rhd-plugin-choice-plan.md`](../rhd-plugin-choice-plan.md)

## Overview

Create a new plugin crate `plugins/rhd_plugin_choice` that registers with the chat
server and ensures the `rhd_choice` tool exists in **every** chat — newly created
ones and pre-existing ones (startup reconciliation). The plugin deliberately has
**no tool-call handler**: answering is the frontend's job (Phase 3).

**Scope in:** crate scaffold, tool definition template, per-chat tool registration,
plugin README, unit tests, workspace membership.
**Scope out:** any `on_tool_call` / custom-event handling, frontend changes.

**Dependencies:** none. Parallel with Phase 2. Must complete before Phase 4.

## Wire Contract (fixed — other phases rely on this)

| Item | Value |
|---|---|
| Tool name | `rhd_choice` |
| Params | `{ question: string, options: string[] }` (both required) |
| Plugin ID (default) | `choice` |
| Answer | `addMessage` with `role: "tool"`, `toolCallId`, `content` = exact option text or user-typed message — posted by the **frontend**, never by this plugin |

## Files to Create/Modify

### 1. `plugins/rhd_plugin_choice/Cargo.toml` (new)

Mirror `plugins/rhd_plugin_todo_list/Cargo.toml`, minus the parser-only deps
(`regex`, `tempfile`, `chrono` are not needed):

```toml
[package]
name = "rhd_plugin_choice"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "rhd_plugin_choice"
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
```

### 2. `Cargo.toml` (root, modify)

Add to `[workspace] members` after `"plugins/rhd_plugin_mcp"`:

```toml
    "plugins/rhd_plugin_choice",
```

### 3. `templates/mcp_internal/rhd_choice/tool_definition.json` (new)

Same shape as `templates/mcp_internal/rhd_set_todo_list/tool_definition.json`:

```json
{
  "type": "function",
  "function": {
    "name": "rhd_choice",
    "description": "Ask the user to decide between concrete options before continuing. Use this tool whenever the next step depends on a user decision with multiple reasonable variants. Provide a short question and a small set of distinct, self-explanatory options. The user's answer is returned as the tool result: either the exact text of the chosen option, or a free-form message the user typed instead. Do not call this tool for yes/no confirmations that you can reasonably proceed on.",
    "parameters": {
      "type": "object",
      "properties": {
        "question": {
          "type": "string",
          "description": "The question shown to the user, phrased so it is understandable on its own"
        },
        "options": {
          "type": "array",
          "items": { "type": "string" },
          "description": "The variants the user can choose between; each option is rendered as a button and its text is returned verbatim when chosen"
        }
      },
      "required": ["question", "options"]
    }
  }
}
```

### 4. `plugins/rhd_plugin_choice/src/lib.rs` (new)

```rust
//! Choice plugin library.
//!
//! This module exports the plugin functionality for testing and reuse.

pub mod plugin;
pub mod templates;
```

### 5. `plugins/rhd_plugin_choice/src/templates.rs` (new)

Trimmed copy of `rhd_plugin_todo_list/src/templates.rs` — only the tool
definition template (no contract/error/env templates):

```rust
//! Template loading.

use include_dir::{include_dir, Dir};

// Embed templates directory at compile time
static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates");

/// Tool definition template for rhd_choice.
pub const TOOL_DEFINITION_TEMPLATE: &str = "mcp_internal/rhd_choice/tool_definition.json";

/// Loaded templates.
#[derive(Debug, Clone)]
pub struct Templates {
    tool_definition: String,
}

impl Templates {
    /// Load all templates from embedded files.
    pub fn load() -> Result<Self, TemplateError> {
        Ok(Self {
            tool_definition: Self::load_file(TOOL_DEFINITION_TEMPLATE)?,
        })
    }

    fn load_file(path: &str) -> Result<String, TemplateError> {
        TEMPLATES_DIR
            .get_file(path)
            .ok_or_else(|| TemplateError::NotFound(path.to_string()))?
            .contents_utf8()
            .map(|s| s.to_string())
            .ok_or_else(|| TemplateError::InvalidEncoding(path.to_string()))
    }

    /// Get the tool definition JSON.
    pub fn tool_definition(&self) -> &str {
        &self.tool_definition
    }

    /// Parse tool definition as JSON value.
    pub fn tool_definition_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.tool_definition)
    }
}

/// Errors that can occur during template loading.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("template not found: {0}")]
    NotFound(String),
    #[error("invalid encoding in template: {0}")]
    InvalidEncoding(String),
}
```

### 6. `plugins/rhd_plugin_choice/src/main.rs` (new)

Mirror `rhd_plugin_todo_list/src/main.rs` (tracing init, clap, run):

```rust
//! Choice plugin for RHD chat system.
//!
//! This plugin provides the `rhd_choice` tool to all chats. It registers the
//! tool per chat; answering the tool calls is the frontend's responsibility.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod plugin;
mod templates;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_choice")]
#[command(about = "Choice tool plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "choice")
    #[arg(long, default_value = "choice")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_choice=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_choice");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    plugin::run_plugin(&args.server_url, &args.plugin_id).await?;

    Ok(())
}
```

### 7. `plugins/rhd_plugin_choice/src/plugin.rs` (new)

Lifecycle identical to `rhd_plugin_todo_list/src/plugin.rs` **minus** the
contract message, the `on_tool_call` subscription, and the
`ai_completions:preRequest` handler:

```rust
//! Main plugin logic.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddToolsParams, GetPendingAcksParams, RegisterPluginParams,
    ToolDefinition,
};
use rhd_chat_client::ChatClient;
use tokio::sync::RwLock;

use crate::templates::Templates;

/// Run the choice plugin.
///
/// Lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create chat monitor, subscribe to all chats
/// 5. On every chat state change: register rhd_choice tool once per chat
/// 6. Main loop (keep alive)
pub async fn run_plugin(server_url: &str, plugin_id: &str) -> Result<(), PluginError> {
    let templates = Arc::new(Templates::load().map_err(|e| PluginError::Template(e.to_string()))?);
    tracing::info!("Templates loaded");

    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    tracing::info!("Connected to chat server");

    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    tracing::info!("Registered as plugin: {}", plugin_id);

    // Drain pending acks so other plugins' coordination is never blocked by us.
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;
    tracing::info!("Subscribed to all chats");

    // Chats we already registered the tool for (avoids redundant addTools calls).
    let initialized_chats = Arc::new(RwLock::new(std::collections::HashSet::new()));

    let client_for_chat = Arc::clone(&client);
    let templates_for_chat = Arc::clone(&templates);
    let initialized_chats_for_chat = Arc::clone(&initialized_chats);

    chat_monitor
        .on_chat_state_change(move |chat_id, _chat_state| {
            let client = Arc::clone(&client_for_chat);
            let templates = Arc::clone(&templates_for_chat);
            let initialized_chats = Arc::clone(&initialized_chats_for_chat);

            // NEVER await inline in the monitor's dispatch path — spawn (see
            // memory/development.md "Event Callbacks Must Not Block").
            tokio::spawn(async move {
                {
                    let initialized = initialized_chats.read().await;
                    if initialized.contains(&chat_id) {
                        return;
                    }
                }

                let tool_def: ToolDefinition =
                    match serde_json::from_str(templates.tool_definition()) {
                        Ok(def) => def,
                        Err(e) => {
                            tracing::error!(
                                chat_id = chat_id,
                                error = %e,
                                "failed to parse tool definition"
                            );
                            return;
                        }
                    };

                tracing::info!(chat_id = chat_id, "registering rhd_choice tool");
                if let Err(e) = client
                    .add_tools(AddToolsParams {
                        chat_id,
                        tools: vec![tool_def],
                    })
                    .await
                {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to register tool"
                    );
                    return; // retry on the next state change for this chat
                }

                let mut initialized = initialized_chats.write().await;
                initialized.insert(chat_id);
                tracing::info!(chat_id = chat_id, "tool registered");
            });
        })
        .await;

    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to load templates: {0}")]
    Template(String),
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
}
```

### 8. `plugins/rhd_plugin_choice/README.md` (new)

Follow the README template in `plugins/README.md`. Must document (per the
"Plugin Responsibilities" rule):

- **Overview**: provides `rhd_choice` to all chats so the assistant can ask the
  user to choose between options; the frontend renders the question as buttons
  plus a free-text input and answers the tool call.
- **Trigger conditions**: on every chat state change (new chat or startup
  reconciliation) → `addTools` once per chat.
- **Events listened for**: chat state changes only. **Events emitted**: none.
  **Tags added**: none.
- **Tool contract**: the table above (name, params, answer semantics — answer
  content is the exact option text or the user's typed message).
- **Explicit note**: the plugin never answers tool calls; an unanswered
  `rhd_choice` call intentionally pauses the `ai_completions` tool loop until a
  human responds in the UI.
- **CLI usage**: `rhd_plugin_choice --server-url ws://localhost:8080/ [--plugin-id choice]`
- **Dependencies**: rhd_chat_client, rhd_chat_api.

## Tests

`plugins/rhd_plugin_choice/tests/integration_test.rs` (mirrors the template
tests in `rhd_plugin_todo_list/tests/integration_test.rs`):

```rust
//! Integration tests for the choice plugin.

use rhd_plugin_choice::templates::Templates;

#[test]
fn test_templates_load() {
    let templates = Templates::load();
    assert!(templates.is_ok(), "Failed to load templates: {:?}", templates.err());
}

#[test]
fn test_tool_definition_valid_json() {
    let templates = Templates::load().unwrap();
    let json = templates.tool_definition_json();
    assert!(json.is_ok(), "Tool definition is not valid JSON: {:?}", json.err());

    let json = json.unwrap();
    assert_eq!(json["type"], "function");
    assert_eq!(json["function"]["name"], "rhd_choice");
    assert!(json["function"]["parameters"]["properties"]["question"].is_object());
    assert!(json["function"]["parameters"]["properties"]["options"].is_object());
    let required = json["function"]["parameters"]["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("question")));
    assert!(required.contains(&serde_json::json!("options")));
}
```

## Verification

```bash
cargo check -p rhd_plugin_choice
cargo test -p rhd_plugin_choice
```

## Implementation Notes

1. **No contract system message** (unlike `todo_list`): the `rhd_choice` tool is
   fully self-describing via its definition, which reaches the model in every
   request's `tools` array. A per-chat system message would be redundant noise.
2. **Idempotent registration**: server `addTools` does
   `INSERT OR REPLACE INTO chat_tools (chat_id, plugin_id, tool_name, ...)`
   (`packages/rhd_db/src/chat_db/tools.rs`), so plugin restarts / re-delivered
   events are harmless. The in-memory `initialized_chats` set only avoids
   redundant requests; on `add_tools` error we deliberately do **not** mark the
   chat initialized so the next state-change retries.
3. **Startup reconciliation is built in**: `subscribe_to_all_chats` +
   `on_chat_state_change` fire for pre-existing chats too, so chats created
   before the plugin started also get the tool.
4. **`getPendingAcks` drain**: the plugin subscribes to no custom events, but
   still acks pending ones at startup (todo_list pattern) so senders waiting on
   all-plugin acknowledgments never time out on us.
5. **Spawned callbacks**: the `on_chat_state_change` closure body is
   `tokio::spawn`ed — inline `await` of a client request from the monitor's
   dispatch path deadlocks the WebSocket read task
   (`memory/development.md` "Event Callbacks Must Not Block the WebSocket Read
   Task").

## Dependencies

- Depends on: nothing.
- Blocks: Phase 4 (validation). Phase 3 consumes only the **Wire Contract**
  above, not this code — it can proceed in parallel.
