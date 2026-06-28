# Chat AI Streaming Plan

## Goal

Add streaming (SSE) support to `OpenAiClient` in `rhd_ai` crate and make `ChatMessage` public/cloneable for building multi-turn message histories.

## Scope

- New `chat_stream()` method on `OpenAiClient` that accepts message history and callback for chunks
- SSE parsing of `text/event-stream` response
- Abort support via callback returning `false` or cancellation token
- Refactor `ChatMessage` to be public, owned (not borrowed), and `Clone`
- Preserve existing `chat()` and `chat_with_tools()` APIs

## Public API

```rust
// Refactored — owned, cloneable, public
#[derive(Clone, Debug)]
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant {
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self;
    pub fn system(content: impl Into<String>) -> Self;
    pub fn assistant(content: impl Into<String>) -> Self;
}

pub struct StreamChunk {
    pub content: Option<String>,
    pub finish_reason: Option<String>,
}

pub struct StreamResult {
    pub finish_reason: Option<String>,
    pub usage: Option<rhd_api::TokenUsage>,
}

impl OpenAiClient {
    // Existing methods refactored to use new ChatMessage internally
    pub async fn chat(&self, model: &str, system: &str, message: &str) -> Result<String, AiError>;
    pub async fn chat_with_tools(...) -> Result<ChatResult, AiError>;

    // New streaming method
    pub async fn chat_stream<F>(
        &self,
        model: &str,
        messages: &[ChatMessage],
        on_chunk: F,
    ) -> Result<StreamResult, AiError>
    where
        F: FnMut(StreamChunk) -> bool; // return false to abort

    // Cancellation-token variant for cleaner abort
    pub async fn chat_stream_cancellable(
        &self,
        model: &str,
        messages: &[ChatMessage],
        cancel: tokio_util::sync::CancellationToken,
        on_chunk: impl FnMut(StreamChunk),
    ) -> Result<StreamResult, AiError>;
}
```

## File Changes

| File | Change |
|------|--------|
| `packages/rhd_ai/src/client.rs` | Refactor `ChatMessage` to owned/cloneable; add `chat_stream` and `chat_stream_cancellable`; add `StreamChunk`, `StreamResult` |
| `packages/rhd_ai/src/lib.rs` | Re-export new types |
| `packages/rhd_ai/Cargo.toml` | Add `tokio-util` dependency (for `CancellationToken`) if not present |

## Implementation Steps

1. Refactor `ChatMessage` enum:
   - Change from borrowed `&'a str` to owned `String`
   - Derive `Clone`, `Debug`
   - Add convenience constructors (`user()`, `system()`, `assistant()`)
2. Update existing `chat()` and `chat_with_tools()` to build new `ChatMessage` variants
3. Add `StreamChunk` and `StreamResult` structs
4. Implement `chat_stream()`:
   - Build request with `stream: true`
   - Send POST, check status
   - Read response body as byte stream
   - Parse SSE: split by `\n\n`, extract `data:` lines
   - Skip `data: [DONE]` sentinel
   - Parse each chunk JSON → extract `choices[0].delta.content` and `finish_reason`
   - Call `on_chunk` callback; if returns `false`, abort request
5. Implement `chat_stream_cancellable()`:
   - Same as above but check `cancel.is_cancelled()` between chunks
   - Also abort HTTP request on cancellation
6. Handle error cases:
   - Network error during streaming → return `AiError::Network`
   - API error (non-2xx) → return `AiError::Api`
   - Parse error → return `AiError::Parse`
   - Cancellation → return specific error or `Ok` with partial result (decide: prefer `Err(AiError::Aborted)`)
7. Add `AiError::Aborted { model: String }` variant
8. Unit tests with mock server returning SSE stream

## SSE Parsing Details

OpenAI streaming format:
```
data: {"id":"...","choices":[{"delta":{"content":"Hello"},"index":0}]}

data: {"id":"...","choices":[{"delta":{"content":" world"},"index":0}]}

data: {"id":"...","choices":[{"delta":{},"finish_reason":"stop","index":0}],"usage":{...}}

data: [DONE]
```

Parser logic:
- Read line by line
- Skip empty lines and lines starting with `:` (comments)
- For `data:` lines, parse JSON
- If data is `[DONE]`, stop
- Extract `choices[0].delta.content` (may be absent)
- Extract `choices[0].delta.finish_reason` (may be absent)
- Extract `usage` from final chunk if present

## Design Notes

- Use `reqwest::Response::bytes_stream()` for async byte streaming
- Split bytes by `\n` delimiter, buffer partial lines
- `CancellationToken` from `tokio_util` preferred over callback-bool for abort — cleaner separation
- Keep callback variant for simpler use cases
- Existing `chat()` and `chat_with_tools()` must continue working unchanged (backward compat)

## Success Criteria

- [ ] `ChatMessage` is public, owned, cloneable
- [ ] Existing `chat()` and `chat_with_tools()` still work
- [ ] `chat_stream()` receives chunks via callback
- [ ] `chat_stream_cancellable()` aborts on token cancellation
- [ ] SSE parsing handles multi-line chunks, `[DONE]` sentinel, missing fields
- [ ] `AiError::Aborted` variant exists
- [ ] Unit tests pass with mock SSE server

## Dependencies

None — standalone. Backend chat plan depends on this.
