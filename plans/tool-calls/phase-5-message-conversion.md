# Phase 5: `message_conversion` Module — Faithful Requests or No Requests (milestone P6 + P8)

## Overview

The heart of the milestone plan. New module `message_conversion.rs` replaces
`filter_messages_for_ai` + `convert_to_ai_messages` in `ai_request.rs` with a single
validated build:

- **D2** — assistant `tool_calls` are mapped field-for-field to the AI-client type;
  per-call `tags` cannot leak (the AI-client type has no `tags` field — keep it that way).
- **D3** — assistant `reasoning_content` is sent when stored, non-empty, and enabled via
  the new per-model `ModelConfig.sendReasoningContent` flag (default **true**).
- **D4** — the converter **never strips**: it validates the tool-call sequence and returns
  `Result<Vec<ChatMessage>, ConversionError>`. On error `handle_ai_request` sends nothing,
  parks the chat with `ai_completions:error`, and returns `AiRequestError::MessageConversion`.
- **D5** — exactly two policy exclusions (unknown role; error-tagged message); everything
  else is forwarded verbatim.
- **P8** — `process_queued_messages` carries `tool_call_id` when promoting a queued
  message, so a queued tool message passes validation instead of parking the chat.
- **D7** — extraction brings `ai_request.rs` back under the 500-line cap.

## Dependencies

- **Requires phase-1 + phase-2** (`tool_call_id` on `Message`, params, server).
- **Requires phase-3** (`ChatMessage::Assistant.reasoning_content`).
- **Requires phase-4** (the real D4 trigger gate must exist *before* tool calls are
  forwarded — milestone "Risks / Notes": shipping P6 without P5 would park every
  mid-loop chat instead of letting it wait).
- Phase-6 (startup crash tagging) is independent of this phase.

## Files to Modify / Create

### 1. `plugins/rhd_plugin_ai_completions/src/config.rs`

**Extend `ModelConfig`** (line 18) and add an impl:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub alias: Option<String>,
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<ApiKeyConfig>,
    pub model: Option<String>,
    /// Replay assistant `reasoning_content` to the provider (default: true).
    /// Set false for providers that reject or mishandle it (e.g. DeepSeek-R1).
    #[serde(rename = "sendReasoningContent")]
    pub send_reasoning_content: Option<bool>,
}

impl ModelConfig {
    /// D3: reasoning content is sent unless explicitly disabled for this model.
    pub fn sends_reasoning_content(&self) -> bool {
        self.send_reasoning_content.unwrap_or(true)
    }
}
```

**Add a config test** (follow the existing `NamedTempFile` pattern in `mod tests`):

```rust
#[test]
fn test_send_reasoning_content_flag() {
    let mut config_file = NamedTempFile::new().unwrap();
    std::io::Write::write_all(
        &mut config_file,
        br#"
credentialsConfig: credentials.yaml
ai_completions:
  models:
    default:
      alias: strict
    strict:
      baseUrl: "https://example.com/v1"
      apiKey:
        cred: k
      model: "m"
      sendReasoningContent: false
"#,
    )
    .unwrap();
    let config = load_config(config_file.path().to_str().unwrap()).unwrap();
    assert!(!config.ai_completions.models["strict"].sends_reasoning_content());
    assert!(config.ai_completions.models["default"].sends_reasoning_content());
}
```

### 2. `plugins/rhd_plugin_ai_completions/src/message_conversion.rs` — NEW

Complete file:

```rust
//! Chat history → AI request conversion with integrity validation.
//!
//! Core principle: **a request is either complete or it is not sent.** This module
//! renders the stored chat history faithfully:
//!
//! - assistant `tool_calls` are forwarded field-for-field (D2); per-call `tags` are
//!   RHD-internal orchestration metadata and never reach the provider (the AI-client
//!   `ToolCall` type deliberately has no `tags` field),
//! - assistant `reasoning_content` is forwarded when present and enabled per model (D3),
//! - `tool` messages carry their `tool_call_id` (D1),
//! - unknown roles and `ai_completions:error`-tagged messages are the only permitted
//!   exclusions (D5),
//! - any inconsistency in the tool-call sequence aborts the whole build (D4). The
//!   caller then parks the chat instead of sending a lossy request.

use std::collections::HashSet;

use rhd_ai_client::{ChatMessage, FunctionCall as AiFunctionCall, ToolCall as AiToolCall};
use rhd_chat_api::Message;

use crate::config::ModelConfig;

/// Tag marking internal bookkeeping messages; never sent to the provider.
const ERROR_TAG: &str = "ai_completions:error";

/// Reasons `build_chat_messages` refuses to produce a request.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("assistant message {message_id} tool call {tool_call_id} has no matching tool result")]
    UnresolvedToolCall { message_id: i64, tool_call_id: String },
    #[error("tool message {message_id} references undeclared tool call {tool_call_id}")]
    OrphanToolResult { message_id: i64, tool_call_id: String },
    #[error("tool message {message_id} has no tool_call_id")]
    MissingToolCallId { message_id: i64 },
    #[error("assistant message {message_id} has no content, tool calls, or reasoning")]
    EmptyAssistantMessage { message_id: i64 },
}

/// Build the provider request messages from stored chat history.
///
/// `Err` means the history is inconsistent (the D4 trigger gate was bypassed or raced,
/// e.g. another plugin deleted a message between the decision and the build). The caller
/// must send nothing.
pub fn build_chat_messages(
    messages: &[Message],
    model_config: &ModelConfig,
) -> Result<Vec<ChatMessage>, ConversionError> {
    let included: Vec<&Message> = messages.iter().filter(|m| is_included(m)).collect();
    validate_tool_call_integrity(&included)?;
    Ok(included
        .into_iter()
        .filter_map(|m| to_chat_message(m, model_config))
        .collect())
}

/// Policy inclusion (D5): a known role and not an error-tagged bookkeeping message.
fn is_included(message: &Message) -> bool {
    matches!(
        message.role.as_str(),
        "user" | "assistant" | "system" | "tool"
    ) && !message.tags.iter().any(|tag| tag == ERROR_TAG)
}

/// Validate the OpenAI tool-call contract over the included sequence:
/// every declared id is answered before the next assistant turn, every tool message
/// carries an id and references a declared one, and no assistant message is empty.
fn validate_tool_call_integrity(messages: &[&Message]) -> Result<(), ConversionError> {
    let mut declared: HashSet<String> = HashSet::new();
    // (id of the declaring assistant message, tool call id) awaiting a result.
    let mut pending: Vec<(i64, String)> = Vec::new();

    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                if let Some((owner_id, call_id)) = pending.first() {
                    return Err(ConversionError::UnresolvedToolCall {
                        message_id: *owner_id,
                        tool_call_id: call_id.clone(),
                    });
                }
                let has_reasoning = message
                    .reasoning_content
                    .as_deref()
                    .is_some_and(|reasoning| !reasoning.is_empty());
                if message.content.is_empty() && message.tool_calls.is_empty() && !has_reasoning {
                    return Err(ConversionError::EmptyAssistantMessage {
                        message_id: message.id,
                    });
                }
                for tool_call in &message.tool_calls {
                    declared.insert(tool_call.id.clone());
                    pending.push((message.id, tool_call.id.clone()));
                }
            }
            "tool" => {
                let Some(call_id) = message.tool_call_id.as_deref() else {
                    return Err(ConversionError::MissingToolCallId {
                        message_id: message.id,
                    });
                };
                if !declared.contains(call_id) {
                    return Err(ConversionError::OrphanToolResult {
                        message_id: message.id,
                        tool_call_id: call_id.to_string(),
                    });
                }
                pending.retain(|(_, pending_id)| pending_id != call_id);
            }
            _ => {}
        }
    }

    if let Some((owner_id, call_id)) = pending.first() {
        return Err(ConversionError::UnresolvedToolCall {
            message_id: *owner_id,
            tool_call_id: call_id.clone(),
        });
    }
    Ok(())
}

/// Convert one stored message to its provider representation.
///
/// Returns `None` only for policy-excluded messages (D5). It never strips tool-call
/// data: a `tool` message without an id cannot reach the mapping stage because
/// `validate_tool_call_integrity` runs first and rejects it.
pub fn to_chat_message(message: &Message, model_config: &ModelConfig) -> Option<ChatMessage> {
    if !is_included(message) {
        return None;
    }
    match message.role.as_str() {
        "user" => Some(ChatMessage::User {
            content: message.content.clone(),
        }),
        "system" => Some(ChatMessage::System {
            content: message.content.clone(),
        }),
        "assistant" => {
            let tool_calls = if message.tool_calls.is_empty() {
                None
            } else {
                Some(
                    message
                        .tool_calls
                        .iter()
                        .map(|tool_call| AiToolCall {
                            id: tool_call.id.clone(),
                            call_type: tool_call.call_type.clone(),
                            function: AiFunctionCall {
                                name: tool_call.function.name.clone(),
                                arguments: tool_call.function.arguments.clone(),
                            },
                        })
                        .collect(),
                )
            };
            let reasoning_content = if model_config.sends_reasoning_content() {
                message
                    .reasoning_content
                    .clone()
                    .filter(|reasoning| !reasoning.is_empty())
            } else {
                None
            };
            Some(ChatMessage::Assistant {
                content: Some(message.content.clone()),
                tool_calls,
                reasoning_content,
            })
        }
        "tool" => Some(ChatMessage::Tool {
            tool_call_id: message.tool_call_id.clone().unwrap_or_default(),
            content: message.content.clone(),
        }),
        _ => None,
    }
}
```

> Note on the `tool` arm's `unwrap_or_default()`: it is **not** a stripping path — the
> id's presence is guaranteed by the integrity check that runs before any mapping. It
> exists only to keep `to_chat_message` infallible.

**Tests** (append `#[cfg(test)] mod tests` to the same file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::{FunctionCall, ToolCall};

    fn model_config(send_reasoning: Option<bool>) -> ModelConfig {
        ModelConfig {
            alias: None,
            base_url: None,
            api_key: None,
            model: None,
            send_reasoning_content: send_reasoning,
        }
    }

    fn message(id: i64, role: &str, content: &str) -> Message {
        Message {
            id,
            chat_id: 1,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        }
    }

    fn assistant_with_calls(id: i64, call_ids: &[&str]) -> Message {
        let mut msg = message(id, "assistant", "");
        msg.tool_calls = call_ids
            .iter()
            .map(|call_id| ToolCall {
                id: call_id.to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: format!("{{\"city\":\"{call_id}\"}}"),
                },
                tags: vec!["pending".to_string()],
            })
            .collect();
        msg
    }

    fn tool_result(id: i64, tool_call_id: &str) -> Message {
        let mut msg = message(id, "tool", "result");
        msg.tool_call_id = Some(tool_call_id.to_string());
        msg
    }

    fn default_config() -> ModelConfig {
        model_config(None)
    }

    #[test]
    fn assistant_tool_calls_are_forwarded_verbatim() {
        let messages = vec![
            message(1, "user", "Weather in Paris and London?"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_a"),
            tool_result(4, "call_b"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();

        match &built[1] {
            ChatMessage::Assistant {
                content, tool_calls, ..
            } => {
                assert_eq!(content, &Some(String::new()));
                let calls = tool_calls.as_ref().expect("tool calls forwarded");
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].id, "call_a");
                assert_eq!(calls[0].call_type, "function");
                assert_eq!(calls[0].function.name, "get_weather");
                assert_eq!(calls[0].function.arguments, "{\"city\":\"call_a\"}");
                assert_eq!(calls[1].id, "call_b");
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        // Per-call tags are RHD-internal and must not reach the provider.
        let json = serde_json::to_string(&built).unwrap();
        assert!(!json.contains("pending"));
        assert!(!json.contains("\"tags\""));
    }

    #[test]
    fn tool_messages_carry_their_tool_call_id() {
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_a"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        match &built[1] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_a");
                assert_eq!(content, "result");
            }
            other => panic!("expected tool, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_content_sent_when_enabled_and_present() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some("deep thought".to_string());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &model_config(Some(true))).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &Some("deep thought".to_string()));
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_content_suppressed_when_disabled() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some("deep thought".to_string());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &model_config(Some(false))).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &None);
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        let json = serde_json::to_string(&built).unwrap();
        assert!(!json.contains("reasoning_content"));
    }

    #[test]
    fn empty_reasoning_content_is_skipped() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some(String::new());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &None);
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn unknown_role_is_excluded_not_coerced() {
        let messages = vec![
            message(1, "user", "Hello"),
            message(2, "ai_completions:something", "noise"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 1);
        assert!(matches!(built[0], ChatMessage::User { .. }));
    }

    #[test]
    fn error_tagged_assistant_is_excluded() {
        // The latent bug this fixes: error text used to be replayed as a normal
        // assistant turn because filtering was by role, not tag.
        let mut broken = message(2, "assistant", "AI request failed: timeout");
        broken.tags = vec!["ai_completions:error".to_string()];
        let messages = vec![message(1, "user", "Hello"), broken];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 1);
    }

    #[test]
    fn unresolved_tool_call_aborts_build() {
        let messages = vec![
            message(1, "user", "Use tools"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_a"),
        ];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        match err {
            ConversionError::UnresolvedToolCall { message_id, tool_call_id } => {
                assert_eq!(message_id, 2);
                assert_eq!(tool_call_id, "call_b");
            }
            other => panic!("expected UnresolvedToolCall, got {other:?}"),
        }
    }

    #[test]
    fn orphan_tool_result_aborts_build() {
        let messages = vec![
            message(1, "user", "hi"),
            tool_result(2, "call_never_declared"),
        ];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        match err {
            ConversionError::OrphanToolResult { message_id, tool_call_id } => {
                assert_eq!(message_id, 2);
                assert_eq!(tool_call_id, "call_never_declared");
            }
            other => panic!("expected OrphanToolResult, got {other:?}"),
        }
    }

    #[test]
    fn missing_tool_call_id_aborts_build() {
        let messages = vec![message(1, "tool", "result without id")];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        assert!(matches!(
            err,
            ConversionError::MissingToolCallId { message_id: 1 }
        ));
    }

    #[test]
    fn empty_assistant_message_aborts_build() {
        let messages = vec![message(1, "user", "hi"), message(2, "assistant", "")];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        assert!(matches!(
            err,
            ConversionError::EmptyAssistantMessage { message_id: 2 }
        ));
    }

    #[test]
    fn assistant_with_only_tool_calls_is_not_empty() {
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_a"),
        ];
        assert!(build_chat_messages(&messages, &default_config()).is_ok());
    }

    #[test]
    fn consistent_history_never_errors() {
        let messages = vec![
            message(1, "system", "You are helpful"),
            message(2, "user", "Weather?"),
            assistant_with_calls(3, &["call_a", "call_b"]),
            tool_result(4, "call_a"),
            tool_result(5, "call_b"),
            message(6, "assistant", "Sunny and mild"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 6);
    }
}
```

### 3. `plugins/rhd_plugin_ai_completions/src/queued_messages.rs` — NEW

Move `process_queued_messages` out of `ai_request.rs` verbatim (it is the second-largest
block there and is only needed to keep `ai_request.rs` under the line cap), adding the
**P8** carry-through:

```rust
//! Promotion of queued messages into the conversation before a request is built.

use rhd_chat_api::{AddMessageParams, DeleteQueueMessageParams, GetQueueMessagesParams};
use rhd_chat_client::ChatClient;

use crate::ai_request::AiRequestError;

/// Process queued messages: remove from queue and add as regular messages.
///
/// `tool_call_id` is carried through (P8) so a queued `tool` message keeps the id of
/// the call it answers and passes the D4 integrity check after promotion.
pub async fn process_queued_messages(
    client: &ChatClient,
    chat_id: i64,
) -> Result<(), AiRequestError> {
    // ... identical body to the current ai_request.rs:430-489, with ONE change:
    // the AddMessageParams literal gains the field:
    //
    //     client
    //         .add_message(AddMessageParams {
    //             chat_id,
    //             role: queue_msg.role,
    //             content: queue_msg.content,
    //             tool_call_id: queue_msg.tool_call_id,
    //             reasoning_content: queue_msg.reasoning_content,
    //             tags: queue_msg.tags,
    //             is_finished: true,
    //             is_streaming: false,
    //         })
    //         .await
    //         ...
}
```

> `queue_msg.tool_call_id` is `Option<String>` on both sides — a plain move, no clone.

### 4. `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Remove** (they are superseded by `message_conversion`):

- `filter_messages_for_ai` (lines 491–501)
- `convert_to_ai_messages` (lines 503–529)
- the entire `#[cfg(test)] mod tests` (lines 558–634 — all three tests exercise the
  removed functions)
- `process_queued_messages` (lines 429–489 — moved to `queued_messages.rs`)
- now-unused imports: `ChatMessage` from the `rhd_ai_client` import,
  `DeleteQueueMessageParams`, `GetQueueMessagesParams`

**Add imports:**

```rust
use crate::message_conversion;
use crate::queued_messages::process_queued_messages;
```

**Replace the request-build section** (current lines 146–154) with model-config-first,
then the validated build. This runs **before** the streaming placeholder message is
created (line 157), so a parked chat leaves no half-written assistant message:

```rust
    // Get model config
    let default_model = &config.ai_completions.models["default"];
    let default_name = "default".to_string();
    let model_name = default_model.alias.as_ref().unwrap_or(&default_name);
    let model_config = &config.ai_completions.models[model_name];

    // Build AI request. A request is either complete or it is not sent (D4): an
    // inconsistent history parks the chat instead of producing a stripped request.
    let ai_messages = match message_conversion::build_chat_messages(&current_messages, model_config) {
        Ok(built) => built,
        Err(error) => {
            tracing::error!(
                chat_id = chat_id,
                error = %error,
                "message conversion failed; refusing to send request, parking chat"
            );
            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|tag_error| {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %tag_error,
                        "failed to park chat after conversion error"
                    );
                    AiRequestError::TagAdd(tag_error.to_string())
                })?;
            return Err(AiRequestError::MessageConversion(error.to_string()));
        }
    };
```

**Extend `AiRequestError`:**

```rust
    #[error("message conversion failed: {0}")]
    MessageConversion(String),
```

**Update the doc comment** at the top of `handle_ai_request`: step 5 becomes "Build and
validate the AI completion request — on validation failure, park the chat and send
nothing (D4)".

### 5. `plugins/rhd_plugin_ai_completions/src/lib.rs`

```rust
pub mod ai_request;
pub mod config;
pub mod message_conversion;
pub mod plugin;
pub mod queued_messages;
pub mod tool_resolution;
pub mod trigger_detection;
```

## Line-count budget (D7)

`ai_request.rs` is 635 lines. Removals: `process_queued_messages` ≈ 61,
`filter_messages_for_ai` + `convert_to_ai_messages` ≈ 39, test module ≈ 77 → ≈ 458.
Additions: validated build block ≈ +20, error variant +2, imports ±0 → **≈ 480 lines**,
under the 500 cap in [`memory/development.md`](../../memory/development.md). Verify with
`mise run check-large-files`.

## Tests

### Unit tests

The `message_conversion` tests above cover every acceptance bullet of milestone P6:
tool_calls forwarded verbatim (incl. tags never reaching the provider), reasoning
sent/suppressed/skipped-when-empty, unknown role excluded, error-tagged excluded,
multi-turn round-trip, each `ConversionError` variant raised with the right ids, and a
consistent history never erroring.

### Integration test — `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

End-to-end proof of the success criteria. `TestEnv` hardcodes `SimpleListener`; refactor
it to also accept a pre-built listener:

```rust
impl TestEnv {
    async fn new() -> Self {
        let listener = RecordingListener::new();
        Self::with_listener(listener).await
    }

    async fn with_listener(listener: RecordingListener) -> Self {
        // existing body, but `MockAiProvider::start(listener)` instead of SimpleListener
    }
}
```

(`RecordingListener` — `packages/rhd_mock_ai_provider/src/listener.rs:79` — records every
`ChatCompletionRequest`; `last_request()` returns `Option<ChatCompletionRequest>`.)

> Extend the test file's imports: `AddMessageParams`, `UpdateMessageParams` from
> `rhd_chat_api`, and `RecordingListener` from `rhd_mock_ai_provider` (the file currently
> imports `SimpleListener`). `ChatClient::connect(&env.chat_server_url())` is the
> established client-helper pattern in this file.

New test:

```rust
#[tokio::test]
async fn test_tool_loop_history_reaches_provider_intact() {
    init_tracing();
    timeout(Duration::from_secs(10), async {
        let listener = RecordingListener::new();
        listener.push_text("final answer");
        let env = TestEnv::with_listener(listener.clone()).await;

        let plugin_handle = tokio::spawn({
            let url = env.chat_server_url();
            let config = env.config.clone();
            async move { plugin::run_plugin(&url, "test_plugin", config).await }
        });

        let client = ChatClient::connect(&env.chat_server_url()).await.unwrap();
        let chat_id = client
            .create_chat(CreateChatParams { title: "loop".into(), tags: vec![] })
            .await
            .unwrap()
            .chat_id;

        // Assistant declares one tool call (stored via updateMessage, like the plugin does).
        let assistant = client
            .add_message(AddMessageParams {
                chat_id,
                role: "assistant".to_string(),
                content: String::new(),
                tool_call_id: None,
                reasoning_content: Some("thinking".to_string()),
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .unwrap()
            .message_id;
        client
            .update_message(UpdateMessageParams {
                message_id: assistant,
                content: None,
                reasoning_content: None,
                role: None,
                add_tags: vec![],
                remove_tags: vec![],
                is_finished: None,
                is_streaming: None,
                tool_calls: Some(
                    r#"[{"id":"call_a","type":"function","function":{"name":"get_weather","arguments":"{}"},"tags":["pending"]}]"#.to_string(),
                ),
            })
            .await
            .unwrap();

        // A tool-executing plugin posts the result — NEW capability from phases 1-2.
        client
            .add_message(AddMessageParams {
                chat_id,
                role: "tool".to_string(),
                content: "Sunny".to_string(),
                tool_call_id: Some("call_a".to_string()),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .unwrap();

        // Wait for the plugin to trigger and the mock to record the request.
        let request = loop {
            if let Some(req) = listener.last_request() {
                break req;
            }
            sleep(Duration::from_millis(100)).await;
        };

        // Assistant tool_calls forwarded; tool result carries the id; reasoning sent (default on).
        let assistant_msg = request
            .messages
            .iter()
            .find(|m| matches!(m, rhd_ai_client::ChatMessage::Assistant { tool_calls: Some(_), .. }))
            .expect("assistant with tool_calls in request");
        let tool_msg = request
            .messages
            .iter()
            .find(|m| matches!(m, rhd_ai_client::ChatMessage::Tool { .. }))
            .expect("tool result in request");
        match tool_msg {
            rhd_ai_client::ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_a");
                assert_eq!(content, "Sunny");
            }
            _ => unreachable!(),
        }
        let _ = assistant_msg;
        assert!(!request.messages.iter().any(|m| {
            serde_json::to_string(m).unwrap().contains("pending")
        }));

        plugin_handle.abort();
    })
    .await
    .unwrap();
}
```

> Also add the negative case: same history **without** the tool result → the plugin must
> never send (poll `listener.get_requests().is_empty()` for a couple of seconds while
> asserting the chat has no error tag — it waits, it does not park).

## Implementation Notes

1. **Why validation precedes mapping**: `to_chat_message` stays a pure `Option` mapper
   (D5's "unknown role → None"), so the infallible mapping can assume the tool-call
   contract already holds. The two passes read cleanly and each error variant names the
   offending `message_id` (and tool-call id where applicable) for operators.
2. **Parking reuses the existing mechanism** (`update_chat` + `ai_completions:error`, same
   shape as the failed-response path at `ai_request.rs:394-405`), so
   `has_error_tag` suppresses re-triggering — no spin against a provider 400.
3. **The empty-assistant rule ignores the `send_reasoning_content` flag**: a stored
   assistant message with nothing in it is corruption regardless of what we choose to
   send.
4. **`content: Some(m.content)` always** (D3): an assistant with only tool calls sends
   `content: ""`, which providers accept; we never emit a content-less assistant object.
5. **`call_type` passthrough** (D2): the plugin writes `"function"` at
   `ai_request.rs:348`, so the value round-trips unchanged.
6. **Do not add `tags` to `rhd_ai_client::ToolCall`** — the absence is the guarantee that
   per-call tags never reach the provider.
7. **`tools: None`** on `ChatCompletionRequest` stays a known TODO (milestone "Risks /
   Notes") — wiring `getTools` → `ToolDefinition` is out of scope.

## Validation

```bash
mise run check-cargo
mise run test-cargo
mise run check-large-files   # ai_request.rs must be < 500 lines now
```
