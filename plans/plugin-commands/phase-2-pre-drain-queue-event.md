# Phase 2: `ai_completions:preDrainQueue` Custom Event

## Overview

The AI completions plugin emits a new custom event **`ai_completions:preDrainQueue`** on its chat, waits for all other plugins to acknowledge it, and only then moves queued messages into the conversation (`process_queued_messages`). This gives plugins (specifically `rhd_plugin_commands` from Phase 4) a coordination point to inspect and rewrite queued messages before they become permanent history.

**In scope:** emission + ack-wait in `rhd_plugin_ai_completions`, module/plugin doc updates, ai_completions integration tests.
**Out of scope:** server/DB changes (done — Phase 1), the commands plugin itself (Phases 3–4).

**Semantics:**
- Event fires **only** when `trigger_reason == TriggerReason::QueuedMessages`, **after** the `ai_completions:preRequest` ack wait and **immediately before** the drain in `handle_ai_request`.
- Payload mirrors `preRequest`: top-level `chat_id` (stringified), `additional` JSON `{"triggerReason": "queuedMessages"}`.
- Sender waits for acks from every other registered plugin with the same 30 s timeout used by `preRequest`; a timeout follows the existing failure path (error returned → chat parked with `ai_completions:error`).
- Every plugin already acknowledges custom events it does not handle (documented contract in `plugins/README.md`), so when no commands-aware plugin is present the event completes immediately and the drain proceeds as before.

**Dependencies:** none (can be built in parallel with Phases 1 and 3). Required before Phase 4's e2e and Phase 5.

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Add helper** (private to the module) that factors the existing send → record-event-id → wait-for-acks sequence used for `preRequest` (currently lines 47–76):

```rust
/// Emit a custom event for this chat and wait for every other plugin to acknowledge it.
async fn emit_and_wait_for_acks(
    client: &ChatClient,
    plugins_monitor: &PluginsMonitor,
    plugin_id: &str,
    chat_id: i64,
    event_name: &str,
    additional: serde_json::Value,
) -> Result<(), AiRequestError> {
    let event_result = client
        .send_custom_event(SendCustomEventParams {
            event_name: event_name.to_string(),
            additional: Some(
                serde_json::to_string(&additional)
                    .map_err(|e| AiRequestError::EventSend(e.to_string()))?,
            ),
            chat_id: Some(chat_id.to_string()),
            message_id: None,
            tool_call_id: None,
        })
        .await
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;

    plugins_monitor
        .wait_for_acks_except(
            &event_result.event_id,
            &[plugin_id],
            Duration::from_secs(30),
        )
        .await
        .map_err(|e| AiRequestError::WaitTimeout(e.to_string()))?;

    Ok(())
}
```

**Refactor the `preRequest` block** (lines 47–76) to call the helper with `"ai_completions:preRequest"` and the current `{"triggerReason": ...}` JSON — no behavior change, keeps the flow readable.

**Modify — the `QueuedMessages` branch** (currently starting at line 79). Insert the new event between the branch entry and the drain call:

```rust
let current_messages = if trigger_reason == TriggerReason::QueuedMessages {
    tracing::info!(
        chat_id = chat_id,
        trigger_reason = ?trigger_reason,
        "processing queued messages"
    );

    // Give plugins a chance to inspect/rewrite the queue before promotion.
    emit_and_wait_for_acks(
        &client,
        &plugins_monitor,
        plugin_id,
        chat_id,
        "ai_completions:preDrainQueue",
        serde_json::json!({ "triggerReason": "queuedMessages" }),
    )
    .await?;

    process_queued_messages(&client, chat_id).await?;
    // ... (refetch as today, unchanged)
```

**Update doc comments:** the module header flow list (lines 3–8) becomes:

```
//! 1. Send preRequest event and wait for acknowledgments
//! 2. If queuedMessages trigger: send preDrainQueue event and wait for acknowledgments,
//!    then process queued messages (move from queue to regular messages)
//! 3. Build and validate AI request from chat messages (D4: refuse inconsistent histories)
//! 4. Make AI completion request
//! 5. Handle response (success or error)
```

and the `handle_ai_request` doc list (lines 29–35) likewise.

### 2. `plugins/rhd_plugin_ai_completions/README.md`

**Add to "Events Emitted"** (after the `ai_completions:preRequest` section, matching its format):

```markdown
### `ai_completions:preDrainQueue`

Emitted when a queuedMessages-triggered request is about to move queue messages
into the conversation — after `ai_completions:preRequest` acks and before the drain.

**Payload**:
{
  "chatId": 123,
  "triggerReason": "queuedMessages"
}

**Purpose**: Allows plugins to inspect and rewrite queued messages (parse
slash-commands, insert/remove queued messages, adjust tags) before they become
permanent history. The `rhd_plugin_commands` plugin is the primary consumer.

**Emitted only** on the `queuedMessages` trigger path (never on tool-loop
continuation, where there is no queue to drain).

**Wait Logic**: Waits for all other plugins to acknowledge (30 s timeout;
timeout parks the chat with `ai_completions:error`, same as `preRequest`).
```

## Tests

Add to `plugins/rhd_plugin_ai_completions/tests/integration_test.rs` (reuse `TestEnv`, `init_tracing`, and the existing flow of `test_plugin_triggers_on_queued_messages`). Every test registers an observer connection **before** queueing the message so it is in the ack set:

```rust
/// Connect, register as a plugin, and stream custom events to a receiver,
/// acknowledging each one.
async fn spawn_observer(
    url: &str,
    plugin_id: &str,
) -> (Arc<ChatClient>, mpsc::UnboundedReceiver<CustomEventData>) { /* register_plugin + on_custom_event + ack each event, forward event to channel */ }
```

1. **`test_pre_drain_queue_emitted_on_queued_trigger`** — observer registered; queue `"/hello"`; wait for assistant reply; assert the observer saw `ai_completions:preRequest` followed by `ai_completions:preDrainQueue` for the chat, with `chat_id == Some(chat.to_string())` and `additional` containing `"triggerReason":"queuedMessages"`; assert the ack of `preDrainQueue` precedes the first `messageAdded`/assistant appearance (ordering via event timestamps/recording order).
2. **`test_pre_drain_queue_not_emitted_on_tool_loop`** — model on `tool_call_e2e_test.rs`: assistant answers with a tool call, test client posts the tool result, loop continues; assert the observer saw a second `preRequest` (trigger `toolLoopContinuation`) but **no** second `preDrainQueue`, and none at all if no message was queued.
3. **`test_drain_waits_for_pre_drain_queue_acks`** — observer registers plugin `ack_hold`; it acks `preRequest` immediately but holds `preDrainQueue` (do not call `ack_custom_event` until signalled). Queue a message; assert (poll `get_chat` / mock-provider request log) that no drain/assistant message occurs within ~2 s; then `client.ack_custom_event(AckCustomEventParams { event_id, is_rejected: None })`; assert the drain completes and the assistant reply lands — all comfortably under the 30 s wait so the request is not parked.
4. **Regression guard:** all existing ai_completions tests still pass unchanged (no other plugins registered in those tests → `wait_for_acks_except` returns at once).

## Implementation Notes

1. **Event name uses the `ai_completions:` prefix** (underscore) to match `ai_completions:preRequest`, per plan review; the suffix `preDrainQueue` is camelCase.
2. **Why wait, not fire-and-forget:** the whole point is that the drain must not start until every plugin had its chance to rewrite the queue; the ack-wait is the established coordination primitive (`wait_for_acks_except`, same 30 s constant as `preRequest`).
3. **Why after `preRequest` rather than before:** `preRequest` consumers (system_prompt, todo_list) inject regular messages; `preDrainQueue` consumers mutate the queue itself. Keeping the existing event untouched avoids regressing its consumers, and queue rewriting must happen as late as possible (just before promotion).
4. **Failure semantics:** a plugin that never acks now parks the chat on this path too. This is intentional (identical to `preRequest`) — flagged as a risk in the milestone plan; Phase 5 e2e indirectly audits that all in-repo plugins ack unhandled events.
5. **The drain re-reads the queue afterwards** (`process_queued_messages` fetches fresh), so any insertions made by plugins during the preDrainQueue window are picked up — no caching hazard. Phase 1's `ORDER BY position, id` gives commands-inserted prompts their intended place.
6. **Do not acknowledge your own event before waiting** — the pattern is: other plugins ack; the sender acks its own event later (existing code acknowledges own `preRequest` right before adding the running tag; mirror nothing here — `preDrainQueue` needs no self-ack change, `wait_for_acks_except` already excludes self).

## Dependencies

- Depends on: nothing (Phase 1 not required — the event is just a signal).
- Blocks: Phase 4 (executor handles this event), Phase 5 (full e2e).
- Parallel with: Phases 1 and 3.
