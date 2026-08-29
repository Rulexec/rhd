# Phase 3: AI Client — Assistant `reasoning_content` (milestone P4)

## Overview

Add `reasoning_content: Option<String>` to the `Assistant` variant of
[`ChatMessage`](../../packages/rhd_ai_client/src/types.rs:20) so the plugin can replay a
model's stored reasoning back to the provider (decision **D3**). The field is serialized as
`reasoning_content` and **skipped when `None`**, so requests without reasoning are byte-for-byte
what they are today.

Provider gating (`ModelConfig.sendReasoningContent`) is **not** in this phase — it lives in
the plugin's config and is consumed by the converter in phase-5. This phase only makes the
wire type capable of carrying the field.

## Dependencies

- **Independent** of phases 1–2 (different crate); can run in parallel with them.
- **Blocks phase-5**: `build_chat_messages` constructs `ChatMessage::Assistant` with
  `reasoning_content`.

## Files to Modify

### 1. `packages/rhd_ai_client/src/types.rs`

**Modify `ChatMessage::Assistant`** (line 20):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
        /// Model reasoning/thinking content, replayed only when the plugin decides to send it.
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}
```

**Add a `#[cfg(test)] mod tests`** at the end of the file (the file currently has no tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn assistant(content: Option<&str>, reasoning: Option<&str>) -> ChatMessage {
        ChatMessage::Assistant {
            content: content.map(str::to_string),
            tool_calls: None,
            reasoning_content: reasoning.map(str::to_string),
        }
    }

    #[test]
    fn assistant_serializes_reasoning_content_when_set() {
        let msg = assistant(Some("answer"), Some("thinking hard"));
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "answer");
        assert_eq!(json["reasoning_content"], "thinking hard");
    }

    #[test]
    fn assistant_omits_reasoning_content_when_none() {
        let json = serde_json::to_value(assistant(Some("answer"), None)).unwrap();
        assert!(json.get("reasoning_content").is_none());
        assert_eq!(json["role"], "assistant");
    }

    #[test]
    fn assistant_round_trips_all_fields() {
        let msg = ChatMessage::Assistant {
            content: Some("answer".to_string()),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            reasoning_content: Some("because".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn role_tagging_unaffected_for_other_variants() {
        let user = serde_json::to_value(ChatMessage::User { content: "hi".into() }).unwrap();
        assert_eq!(user["role"], "user");
        let tool = serde_json::to_value(ChatMessage::Tool {
            tool_call_id: "call_1".into(),
            content: "ok".into(),
        })
        .unwrap();
        assert_eq!(tool["role"], "tool");
        assert_eq!(tool["tool_call_id"], "call_1");
    }
}
```

### 2. Compile fixes at `ChatMessage::Assistant` construction sites

Adding a field to a struct variant breaks every literal that constructs or exhaustively
matches it. The compiler surfaces these (verified current sites):

- `plugins/rhd_plugin_ai_completions/src/ai_request.rs:513` (`convert_to_ai_messages`) —
  add `reasoning_content: None,`. This is a **temporary** placeholder; the whole function is
  deleted in phase-5.
- `packages/rhd_mock_ai_provider/tests/integration_tests.rs:129` — add `reasoning_content: None,`.
- The match at `plugins/rhd_plugin_ai_completions/src/ai_request.rs:625` already uses
  `{ content, .. }` — no change needed.

## Tests

The four unit tests above. Also run the existing mock-provider test suite to prove the
shared types accept the new field on the request side:

```bash
mise run test-cargo
```

## Implementation Notes

1. **Internally-tagged enums tolerate extra fields** — `#[serde(tag = "role")]` on
   `ChatMessage` serializes struct-variant fields alongside the tag; `skip_serializing_if`
   on `Option` fields works exactly as it already does for `content` / `tool_calls`.
2. **Mock provider needs no change**: `packages/rhd_mock_ai_provider/src/server.rs:98`
   deserializes `Json<ChatCompletionRequest>` using these same types, so it accepts (and
   round-trips) `reasoning_content` automatically.
3. **Position of the field** (`tool_calls` then `reasoning_content`) keeps the existing
   `content`/`tool_calls` order stable for anyone pattern-matching field order in JSON.
4. **Empty-string reasoning is not filtered here** — "only send when non-empty" is a policy
   decision owned by the converter (phase-5), not the wire type.
5. `ChatMessage` derives `PartialEq` — adding `Option<String>` preserves it, so the
   round-trip test compiles.

## Validation

```bash
mise run check-cargo
mise run test-cargo
```
