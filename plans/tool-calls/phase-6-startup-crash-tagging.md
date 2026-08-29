# Phase 6: Startup Crash Detection — Park Chats Left Mid-Stream (milestone P7, decision D6)

## Overview

An assistant message left with `is_streaming == true` or `is_finished == false` can only
mean one thing: the plugin died between
[`add_message`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:157) and the
[`stream_finish`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:326) /
[`update_message`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:359) pair.
The stored content is a truncated, partial answer that must **never** be replayed to the
model as if the model had said it.

**Once at startup**, before the main loop begins polling: for each monitored chat that
contains an unfinished message, add the `ai_completions:error` tag via `update_chat` —
reusing the same tag and the same call shape as the existing failure path
([`ai_request.rs:395-405`](../../plugins/rhd_plugin_ai_completions/src/ai_request.rs:395)).
`has_error_tag` then suppresses every future trigger for that chat, so the partial content
can never reach the provider.

This replaces a per-message streaming guard in the converter: the chat is blocked upstream,
so the converter never sees in-flight messages from a live stream.

**Out of scope (explicitly deferred):** *repairing* those chats — deleting the partial
message, clearing the tag, resuming or discarding the stream. The tag is deliberately
**not** auto-cleared; a crashed chat stays parked, which is the safe state.

## Dependencies

- **Production code is independent** of phases 1–5 (uses only existing `is_streaming` /
  `is_finished` / `update_chat` / `has_error_tag` machinery).
- **Test fixtures require phase-2**: the `Message` / `AddMessageParams` /
  `AddQueueMessageParams` literals below include `tool_call_id`. If this phase is pulled
  forward before phase-2, omit that field from the fixtures (or land it after phase-2).
- Should land **before or with** the docs phase (phase-7 documents this behavior).

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/src/plugin.rs`

**Add imports** to the existing `use` block (lines 12–18):

```rust
use rhd_ai_client::AiClient;
use rhd_chat_api::{
    AckCustomEventParams, GetPendingAcksParams, Message, RegisterPluginParams, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, ChatMonitor};
```

**Insert the reconciliation call** in `run_plugin`, immediately after the
`subscribe_to_all_chats` block (after line 100, `tracing::info!("Subscribed to all chats");`)
and **before** the AI client is created / the main loop starts:

```rust
    // Startup reconciliation (D6): a chat left mid-stream by a previous crash must never
    // trigger again — its partial assistant message must never reach the provider.
    tag_crashed_chats(&client, &chat_monitor).await?;
```

**Add the two functions** after `run_plugin` (before the `PluginError` enum):

```rust
/// Find the first message that proves the plugin crashed mid-stream.
///
/// An assistant message left with `is_streaming == true` or `is_finished == false`
/// can only be the result of a crash between `add_message` and the
/// `stream_finish` / `update_message` pair.
fn find_unfinished_message(messages: &[Message]) -> Option<&Message> {
    messages.iter().find(|m| m.is_streaming || !m.is_finished)
}

/// Once at startup, park every monitored chat that contains an unfinished message
/// by adding the `ai_completions:error` tag (D6).
///
/// `has_error_tag` in `trigger_detection` then suppresses all future triggers for
/// those chats. Chats already carrying the tag are skipped, making this idempotent
/// across restarts.
async fn tag_crashed_chats(
    client: &ChatClient,
    chat_monitor: &ChatMonitor,
) -> Result<(), PluginError> {
    for chat_id in chat_monitor.get_chat_ids().await {
        let Some(state) = chat_monitor.get_chat_state(chat_id).await else {
            continue;
        };

        // Already parked (possibly by a previous run) — nothing to do.
        if state.tags.iter().any(|tag| tag == "ai_completions:error") {
            continue;
        }

        let Some(offender) = find_unfinished_message(&state.messages) else {
            continue;
        };

        tracing::warn!(
            chat_id = chat_id,
            message_id = offender.id,
            is_streaming = offender.is_streaming,
            is_finished = offender.is_finished,
            "unfinished message found at startup; parking chat with error tag"
        );

        client
            .update_chat(UpdateChatParams {
                chat_id,
                title: None,
                add_tags: vec!["ai_completions:error".to_string()],
                remove_tags: vec![],
            })
            .await
            .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
    }

    Ok(())
}
```

**Extend `PluginError`:**

```rust
    #[error("startup reconciliation failed: {0}")]
    StartupReconciliation(String),
```

> `ChatState` is fully populated right after `create_chat_monitor()` +
> `subscribe_to_all_chats()`: the monitor fetches the initial chat list and each chat's
> state during construction ([`chat_monitor.rs:222-236`](../../packages/rhd_chat_client/src/chat_monitor.rs:222)),
> and `get_chat_state` returns a clone with `messages`, `tags`, and
> `queued_messages_count`. No extra fetching is needed here.

## Tests

### Unit tests — `plugin.rs` (append a `#[cfg(test)] mod tests`)

The scan predicate is pure and needs no client:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn message(id: i64, is_finished: bool, is_streaming: bool) -> Message {
        Message {
            id,
            chat_id: 1,
            role: "assistant".to_string(),
            content: "partial".to_string(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
            is_finished,
            is_streaming,
            tool_calls: vec![],
        }
    }

    #[test]
    fn finds_streaming_message() {
        let messages = vec![message(1, true, false), message(2, false, true)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), Some(2));
    }

    #[test]
    fn finds_not_finished_message() {
        let messages = vec![message(1, false, false)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), Some(1));
    }

    #[test]
    fn all_finished_messages_is_none() {
        let messages = vec![message(1, true, false), message(2, true, false)];
        assert_eq!(find_unfinished_message(&messages).map(|m| m.id), None);
    }

    #[test]
    fn empty_history_is_none() {
        assert_eq!(find_unfinished_message(&[]).map(|m| m.id), None);
    }
}
```

### Integration test — `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

Proves the end-to-end parking: a chat with an unfinished message gets the tag and never
triggers. Follow the file's existing `TestEnv` + `tokio::spawn(run_plugin)` pattern.

```rust
#[tokio::test]
async fn test_startup_tags_chat_with_unfinished_message() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let env = TestEnv::new().await;

        // Seed the corrupted state BEFORE the plugin starts: an assistant message
        // left streaming by a simulated crash.
        let client = ChatClient::connect(&env.chat_server_url()).await.unwrap();
        let chat_id = client
            .create_chat(CreateChatParams { title: "crashed".into(), tags: vec![] })
            .await
            .unwrap()
            .chat_id;
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "assistant".to_string(),
                content: "partial answer".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
                is_finished: false,
                is_streaming: true,
            })
            .await
            .unwrap();

        // Now start the plugin — startup reconciliation must park this chat.
        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move { plugin::run_plugin(&url, "test_plugin", config).await }
        });

        // Poll until the error tag appears.
        let tagged = loop {
            let chat = client
                .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
                .await
                .unwrap();
            if chat.chat.tags.iter().any(|t| t == "ai_completions:error") {
                break true;
            }
            if env.elapsed() > Duration::from_secs(5) {
                break false;
            }
            sleep(Duration::from_millis(100)).await;
        };
        assert!(tagged, "chat with unfinished message must be tagged at startup");

        // And it never triggers afterwards: queue a message, wait, assert no AI request.
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id,
                role: "user".to_string(),
                content: "still here?".to_string(),
                tool_call_id: None,
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .unwrap();
        sleep(Duration::from_secs(2)).await;
        let chat = client
            .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
            .await
            .unwrap();
        assert_eq!(
            chat.queued_messages_count, 1,
            "parked chat must not process its queue"
        );

        plugin_handle.abort();
    })
    .await
    .unwrap();
}
```

> The snippet uses a hypothetical `env.elapsed()`; replace it with a simple iteration
> counter or `tokio::time::Instant` — match the waiting style of the neighboring tests
> in this file (they use `sleep` + polling loops). Also add `AddMessageParams` to the
> test file's `rhd_chat_api` imports.

**Also add** the negative case: a chat whose messages are all finished receives **no** tag
and continues to trigger normally (queue a user message, assert the mock receives a
request). And the idempotency case: a chat that already carries `ai_completions:error`
gets no duplicate `update_chat` work — assert the chat `version` does not change across
two plugin restarts (or simply that the tag set is unchanged).

## Implementation Notes

1. **Placement before the loop is load-bearing**: if reconciliation ran inside the poll
   loop, a chat could trigger once (with partial content) before being parked. Running it
   once, before the first `should_trigger` evaluation, closes that window.
2. **Tagging uses the chat-level tag, not a message**: the same
   `update_chat(add_tags: ["ai_completions:error"])` shape as the failed-response path,
   so `trigger_detection::has_error_tag` suppresses the chat with zero new trigger logic.
3. **Idempotent across restarts**: the `state.tags` check skips already-parked chats, so
   repeated restarts don't bump the chat version or spam logs.
4. **The offending `message_id` is logged** (`tracing::warn!`) so operators can see what
   was corrupted — this is the only forensic artifact until repair lands.
5. **Why not a converter guard**: per D6, detection here is cheap and reuses existing
   machinery; a per-message streaming guard in `build_chat_messages` would still let the
   chat trigger repeatedly against a provider 400. Parking upstream is the loud, safe stop.
6. **`?` on the update_chat error**: failing to park is itself a bug-worthy condition;
   aborting `run_plugin` surfaces it at startup rather than silently continuing with a
   chat that could leak partial content.

## Validation

```bash
mise run check-cargo
mise run test-cargo
```
