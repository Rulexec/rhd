# Phase 4: `rhd_plugin_commands` — Event Executor, Plugin Lifecycle, Docs

## Overview

Wire the plugin to the chat server: handle `ai_completions:preDrainQueue` (constant `PRE_DRAIN_QUEUE_EVENT` from Phase 3) by parsing every **user-role** queued message snapshot, executing the recognized command steps in occurrence order, finalizing the carrying messages (strip commands or delete), and always acknowledging. Also write the plugin README and example configuration.

**In scope:** `src/executor.rs`, full `src/plugin.rs` lifecycle, README.md, `config.example.yaml` + sample prompt files, chat-id extraction unit tests.
**Out of scope:** full-stack e2e tests and memory docs (Phase 5), any server/ai_completions changes (Phases 1–2).

**Depends on:**
- Phase 1 — `AddQueueMessageParams.before_message_id` must exist on the wire (`rhd_chat_api`) and be honored by the server.
- Phase 2 — `ai_completions:preDrainQueue` must actually be emitted.
- Phase 3 — `CommandRegistry`, `ResolvedStep`, `parser::parse`, `prompt_tag()`, plugin skeleton.

## Files to Create/Modify

### 1. `plugins/rhd_plugin_commands/src/executor.rs` (new)

```rust
//! Executes queued slash-commands during ai_completions:preDrainQueue.

use std::collections::HashSet;

use rhd_chat_api::{
    AddQueueMessageParams, DeleteQueueMessageParams, GetQueueMessagesParams,
    UpdateChatParams, UpdateQueueMessageParams,
};
use rhd_chat_client::ChatClient;

use crate::config::{CommandRegistry, ResolvedStep};
use crate::parser;
use crate::prompt_tag;

/// Process one chat's queue: parse commands, execute steps, finalize messages.
///
/// Only messages with role "user" from the pre-execution snapshot are parsed —
/// prompt messages inserted during this run are never re-scanned (a prompt file
/// whose text starts with '/' cannot recurse).
pub async fn process_queue(
    client: &ChatClient,
    registry: &CommandRegistry,
    chat_id: i64,
) -> Result<(), ExecutionError> {
    let names: HashSet<String> = registry.names();
    let snapshot = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .map_err(|e| ExecutionError::QueueFetch(e.to_string()))?;

    for msg in snapshot.messages {
        if msg.role != "user" {
            continue;
        }
        let Some(parsed) = parser::parse(&msg.content, &names) else {
            continue; // no recognized commands — leave byte-for-byte untouched
        };

        // 1. Execute steps in occurrence order (multi commands expand in place).
        for invocation in &parsed.invocations {
            let Some(steps) = registry.steps(invocation) else { continue };
            for step in steps {
                if let Err(e) = apply_step(client, registry, chat_id, msg.id, step).await {
                    // Partial application is possible on failure; log with full
                    // context and continue with the remaining messages. The
                    // event is still acknowledged so the chat is never parked.
                    tracing::error!(
                        chat_id = chat_id,
                        message_id = msg.id,
                        command = %invocation,
                        "command step failed: {}", e
                    );
                    break;
                }
            }
        }

        // 2. Finalize the carrying message.
        finalize_message(client, &msg, &parsed.remainder).await?;
    }
    Ok(())
}
```

`apply_step` (one small match — imports from `rhd_chat_api` as listed above):

```rust
async fn apply_step(
    client: &ChatClient,
    registry: &CommandRegistry,   // registry passed only for prompt_tag name reuse
    chat_id: i64,
    message_id: i64,
    step: &ResolvedStep,
) -> Result<(), ExecutionError> {
    match step {
        ResolvedStep::ChatTags { add, remove } => {
            client.update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: add.clone(),
                remove_tags: remove.clone(),
            })
            .await
            .map_err(|e| ExecutionError::ChatTags(e.to_string()))?;
        }
        ResolvedStep::MessageTags { add, remove } => {
            client.update_queue_message(UpdateQueueMessageParams {
                message_id,
                content: None,
                reasoning_content: None,
                role: None,
                add_tags: add.clone(),
                remove_tags: remove.clone(),
            })
            .await
            .map_err(|e| ExecutionError::MessageTags(e.to_string()))?;
        }
        ResolvedStep::Prompt { name, role, content } => {
            // Insert directly before the carrying message. Repeats before the
            // same anchor stay in config order (Phase 1 semantics).
            client.add_queue_message(AddQueueMessageParams {
                chat_id,
                role: role.clone(),
                content: content.clone(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![prompt_tag(name)],
                before_message_id: Some(message_id),
            })
            .await
            .map_err(|e| ExecutionError::PromptInsert(e.to_string()))?;
        }
    }
    tracing::debug!(chat_id, message_id, ?step, "command step applied");
    Ok(())
}
```

`finalize_message`:

```rust
async fn finalize_message(
    client: &ChatClient,
    msg: &rhd_chat_api::Message,
    remainder: &str,
) -> Result<(), ExecutionError> {
    if remainder.trim().is_empty() {
        client.delete_queue_message(DeleteQueueMessageParams { message_id: msg.id })
            .await
            .map_err(|e| ExecutionError::MessageDelete(e.to_string()))?;
        tracing::info!(chat_id = msg.chat_id, message_id = msg.id, "command message consumed");
    } else if remainder != msg.content {
        client.update_queue_message(UpdateQueueMessageParams {
            message_id: msg.id,
            content: Some(remainder.to_string()),
            reasoning_content: None,
            role: None,
            add_tags: vec![],
            remove_tags: vec![],
        })
        .await
        .map_err(|e| ExecutionError::MessageUpdate(e.to_string()))?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("failed to fetch queued messages: {0}")]
    QueueFetch(String),
    #[error("failed to update chat tags: {0}")]
    ChatTags(String),
    #[error("failed to update message tags: {0}")]
    MessageTags(String),
    #[error("failed to insert prompt message: {0}")]
    PromptInsert(String),
    #[error("failed to update message content: {0}")]
    MessageUpdate(String),
    #[error("failed to delete message: {0}")]
    MessageDelete(String),
}
```

`registry` param of `apply_step` is currently unused beyond the tag name carried in `ResolvedStep::Prompt.name` — drop it if it stays unused (avoid a warning).

### 2. `plugins/rhd_plugin_commands/src/plugin.rs` (replace Phase 3 stub body)

Follow the exact lifecycle of `rhd_plugin_system_prompt/src/plugin.rs:22` (connect → register → pending acks → subscriptions → keepalive loop):

```rust
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    registry: CommandRegistry,
) -> Result<(), PluginError> {
    let client = Arc::new(
        ChatClient::connect_with_retry(server_url).await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    client.register_plugin(RegisterPluginParams { plugin_id: plugin_id.to_string() })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    tracing::info!("Registered as plugin: {}", plugin_id);

    // Recovery: process preDrainQueue events missed while disconnected.
    let pending = client.get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    for event in pending.pending_events {
        if event.event_name == crate::PRE_DRAIN_QUEUE_EVENT {
            if let Some(chat_id) = extract_chat_id(&event) {
                if let Err(e) = executor::process_queue(&client, &registry, chat_id).await {
                    tracing::error!(chat_id, "pending preDrainQueue processing failed: {}", e);
                }
            }
        }
        // Ack regardless of name/handling outcome (unhandled events too).
        let _ = client.ack_custom_event(AckCustomEventParams {
            event_id: event.event_id,
            is_rejected: None,
        })
        .await;
    }

    // Live handling. NOTE: rhd_chat_client spawns subscription callbacks
    // (see memory/development.md deadlock pattern) — never block the read task
    // and never await the handler inline.
    let client_for_events = Arc::clone(&client);
    let registry_for_events = Arc::new(registry);
    client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        let registry = Arc::clone(&registry_for_events);
        async move {
            handle_custom_event(&client, &registry, event).await;
        }
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

/// Handle one custom event; returns after acknowledging. NEVER skips the ack.
async fn handle_custom_event(
    client: &ChatClient,
    registry: &CommandRegistry,
    event: CustomEventData,
) {
    let event_id = event.event_id.clone();

    if event.event_name != crate::PRE_DRAIN_QUEUE_EVENT {
        tracing::debug!(event_id = %event_id, event_name = %event.event_name,
            "acknowledging unhandled event");
        let _ = client.ack_custom_event(AckCustomEventParams { event_id, is_rejected: None }).await;
        return;
    }

    let Some(chat_id) = extract_chat_id(&event) else {
        tracing::warn!(event_id = %event_id, "preDrainQueue event missing chatId, acking without action");
        let _ = client.ack_custom_event(AckCustomEventParams { event_id, is_rejected: None }).await;
        return;
    };

    tracing::info!(chat_id, event_id = %event_id, "processing preDrainQueue");
    if let Err(e) = executor::process_queue(client, registry, chat_id).await {
        // Partial failures must not park the chat: log and acknowledge anyway.
        tracing::error!(chat_id, "queue command processing failed: {}", e);
    }
    let _ = client.ack_custom_event(AckCustomEventParams { event_id, is_rejected: None }).await;
}
```

`extract_chat_id(event: &CustomEventData) -> Option<i64>`: copy `extract_chat_id_from_event` from `rhd_plugin_system_prompt/src/plugin.rs` (~lines 330–350, top-level `chat_id` string first, legacy `additional.chatId` fallback) **including its 5 unit tests** from that file's `mod tests` (adjust the `make_event` helper to take an event name).

Add `pub mod executor;` to `lib.rs`. Keep `PluginError::PendingAcks` variant for the new lifecycle.

### 3. `plugins/rhd_plugin_commands/README.md` (new)

Follow the template in `plugins/README.md` ("Plugin README Template") and the section style of `rhd_plugin_system_prompt/README.md`. Must document:

- **Overview** — executes slash-commands embedded at the start of queued user messages just before the AI completions plugin promotes them into the conversation.
- **Trigger Conditions / Events Listened**: `ai_completions:preDrainQueue` (payload: `chatId` top-level, `additional.triggerReason`); all other custom events acknowledged unhandled; `getPendingAcks` recovery on startup.
- **Events Emitted**: none.
- **Commands section** — config format with the canonical four-shape example (from the milestone plan), per-type behavior table, prompt-role rules, path resolution + startup caching, command-name charset.
- **Message syntax** — commands at message start, leading whitespace skipped, whitespace between commands optional (`/a/b` ≡ `/a /b`), unknown token stops parsing and is kept verbatim, message deleted when remainder is empty, tags `commands:prompt:<name>` added to inserted prompt messages and surviving into history.
- **Examples** — reproduce the three walkthrough examples from the milestone plan verbatim (`/prompt_example hello`, `/tags_example /prompt_example`, `/multi_example`).
- **Tags Added**: `commands:prompt:<name>` (messages). Chat-tag effects pass through from configs (no fixed set).
- **Error Handling** — bad config = fail fast at startup; per-step runtime failures logged and skipped; the event is always acknowledged (never parked chats); partial-application limitation on mid-run crash documented.
- **Dependencies** — requires `rhd_plugin_ai_completions` (emitter) and positional queue insert (Phase 1 API).
- **Running**:
  ```bash
  ./rhd_plugin_commands --server-url ws://localhost:8080/ --config commands-config.yaml
  ```

### 4. `plugins/rhd_plugin_commands/config.example.yaml` + sample prompts (new)

The canonical milestone example, with **existing** relative files so the example loads and validates:

```
plugins/rhd_plugin_commands/
  config.example.yaml
  commands/prompt.md
  commands/another_prompt.md
  systemPrompts/warhammer.md
```

## Tests

Unit tests in this phase (full-stack e2e is Phase 5):

1. **`extract_chat_id` set** — the five ported tests from system_prompt (top-level, legacy additional, prefers top-level, missing → None, invalid top-level falls back).
2. **`finalize_message` decision table** via pure helper — extract the branch logic into `fn finalize_plan(current: &str, remainder: &str) -> FinalizeAction { Keep, Delete, Update(String) }` (pure, mirrors the executor branches) and unit-test: `"/a"` remainder `""` → Delete; `"/a hello"` → `Update("hello")`; no-op when equal → Keep.
3. **Step order smoke** — `ResolvedStep` expansion for a multi command: registry built from a temp config; assert `steps("multi_example")` returns `[MessageTags{..}, Prompt{..}, Prompt{..}]` in order (config side; executor order = this slice order).

Manual verification checklist (in README, for the implementer):

```bash
# start server + ai_completions + commands plugin (config.example.yaml), then:
rhd queue add <chat> "/tags_example /prompt_example hello"
rhd queue add <chat> "/multi_example"
rhd queue add <chat> "/notacommand stays"
# expect per milestone plan; check chat tags via `rhd chats` and history via `rhd messages <chat>`
```

## Implementation Notes

1. **Always-ack invariant** is the load-bearing rule: any missed/late ack parks every affected chat for 30 s then errors it. Failures degrade to "commands skipped", never "request blocked" — mirrors the system_prompt plugin's documented policy.
2. **Snapshot iteration** (`getQueueMessages` once per event) is intentional: the queue cannot change between the emitter's wait and our read (drain hasn't started), and our own prompt inserts must not be re-parsed. A concurrent user append during processing lands at the queue end and is simply not command-parsed in this pass — acceptable (it will be parsed next trigger).
3. **No `ChatMonitor` needed**: everything happens inside the event handler with direct fetches. Do not add polling loops (event-driven rule from `memory/development.md`).
4. **Per-message isolation**: a failing message (e.g., step error) `break`s only its own step loop; `process_queue` continues with the next message. The `?` on `finalize_message` is the only early return; if implementers prefer full resilience, downgrade it to a logged error — keep it uniform and note the choice in the code comment.
5. **Tag-then-delete waste**: `message_tags` on a message later deleted (empty remainder) is a no-op round-trip; harmless, keeps ordering semantics simple (do not try to batch steps — one `update_queue_message` call per step keeps occurrence order observable in events).
6. **Idempotency**: stripped messages parse to `None` on re-run, so pending-ack recovery after a crash never re-executes fully-applied commands; partially applied ones may duplicate a prompt insert (documented limitation, matches milestone plan risk register).
7. **Chat tag namespaces are operator-managed** — commands can add/remove any tags (e.g. `pause`, `mcp:common`); this is deliberate (the plugin is a coordination surface, not a policy engine). The `ai_completions:running|error` tags are effectively untouchable in practice because preDrainQueue fires inside the request flow.

## Dependencies

- Depends on: Phase 1 (before_message_id on API + server), Phase 2 (event emitted), Phase 3 (crate skeleton, registry, parser).
- Blocks: Phase 5.
