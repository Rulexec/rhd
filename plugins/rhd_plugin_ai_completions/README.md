# RHD Plugin AI Completions

## Overview

This plugin monitors chats and triggers AI completions when:
1. There are queued messages and no unresolved tool calls
2. All tool calls from the last assistant message are resolved (tool loop continuation)

The plugin automatically processes queued messages, makes AI completion requests, and handles responses including tool calls.

## Trigger Conditions

### Queued Messages Mode

The plugin triggers when:
- Chat has non-zero queued messages count
- Chat has no unresolved tool calls (resolution is matched on tool-call ids: `tool_calls[].id` vs `toolCallId`)
- Chat does not have `ai_completions:error` tag

### Tool Loop Continuation Mode

The plugin triggers when:
- Last assistant message has tool calls
- All tool calls have corresponding tool result messages (resolution is matched on tool-call ids: `tool_calls[].id` vs `toolCallId`)
- Chat does not have `ai_completions:error` tag

## Request Construction

The request sent to the provider is a **faithful rendering of the stored chat history**. Core principle: **a request is either complete or it is not sent.**

- Assistant `tool_calls` are forwarded field-for-field (`id`, `type`, `function.name`, `function.arguments`). Per-call `tags` are RHD-internal orchestration metadata and never reach the provider.
- `tool`-role messages are sent with their `tool_call_id`.
- Assistant `reasoning_content` is sent when stored, non-empty, and enabled for the model (see `sendReasoningContent`).
- Excluded by policy: messages with unknown roles, and messages tagged `ai_completions:error`.

### Integrity validation

Before sending, the tool-call sequence is validated:

- every assistant `tool_calls[].id` is answered by a following `tool` message,
- every `tool` message carries a `tool_call_id` that was previously declared,
- every assistant message has content, tool calls, or reasoning.

A violation means the trigger gate was bypassed or raced (e.g. another plugin deleted a message between the decision and the build). The plugin then **sends nothing**, adds the `ai_completions:error` tag to the chat (parking it, same as a failed response), and logs the offending message and tool-call id. There are no partial requests, ever.

### Unresolved tool calls: no request, keep waiting

While the last assistant message has tool calls without matching results, the plugin does not trigger at all (`TriggerReason::None`). This is a conversation in progress, not a malformed request — the plugin keeps polling until a tool-executing plugin posts the results, then triggers normally.

> Note: no tool-executing plugin exists yet, so a chat whose assistant message has tool calls will simply stop triggering. That is correct, safe behavior.

## Events Emitted

### `ai_completions:preRequest`

Emitted before making AI completion request.

**Payload**:
```json
{
  "chatId": 123,
  "triggerReason": "queuedMessages" | "toolLoopContinuation"
}
```

**Purpose**: Allows other plugins to:
- Add their own messages to the queue
- Modify existing queued messages
- Perform cleanup or logging
- Block the request by not acknowledging

**Wait Logic**: Plugin waits for all other plugins to acknowledge this event before proceeding.

## Tags Added

### `ai_completions:error`

Added to a chat when:
1. an AI request fails, or
2. message conversion refused to build a request (history inconsistency), or
3. at plugin startup, a chat contains a message left unfinished by a crash (`isStreaming` or not `isFinished`) — its partial content must never be replayed.

**When**: see above.

**Effect**: Plugin skips chats with this tag (no further processing). The tag is deliberately not auto-cleared; **repairing crashed chats is not implemented yet**.

### `ai_completions:running`

Added to a chat when the plugin is actively processing an AI request.

**When added**: Right before acknowledging the plugin's own `ai_completions:preRequest` event.

**When removed**:
1. When the AI request completes successfully and the response has no tool calls
2. When transitioning to `ai_completions:error` state (request failure, message conversion failure, streaming error)
3. At plugin startup, if the chat is eligible for a new request

**When kept**:
- When the AI response contains tool calls (tool loop continues)
- If the plugin crashes or fails to acknowledge its own event (chat considered broken until restart)

**Effect**: Provides visibility into when a chat is being processed. Other plugins can use this tag to:
- Display loading indicators in UI
- Avoid conflicting operations on the chat
- Monitor plugin activity

**Startup reconciliation**: At plugin startup, chats with this tag are checked:
- If the chat has an unfinished message (crash recovery) → add `ai_completions:error`, remove `ai_completions:running`
- If the chat is eligible for a request → remove `ai_completions:running` only, let normal flow continue

## Messages Added

### Error Messages

When an AI request fails, the assistant message is updated with:
- **Role**: `assistant` (unchanged)
- **Tags**: `ai_completions:error` added to the message
- **Content**: Error details (error message, timeout info, etc.)

**Filtering**: Messages tagged `ai_completions:error` are excluded from AI requests by policy — they are internal bookkeeping, never model output. Messages with unknown roles are excluded as well. These two are the *only* permitted exclusions; nothing else is ever silently dropped (see "Request Construction").

## Configuration Format

```yaml
credentialsConfig: ../credentials.yaml
ai_completions:
  models:
    default:
      alias: qwen
    qwen:
      baseUrl: "https://api.example.com/v1"
      apiKey:
        cred: alibabaApiKey
      model: "qwen3.7-plus"
      sendReasoningContent: true
```

**Credentials File** (`credentials.yaml`):
```yaml
alibabaApiKey: your-api-key-here
```

### Configuration Fields

- `credentialsConfig`: Path to credentials file (relative or absolute)
- `ai_completions.models`: Map of model configurations
  - `default`: Special key that defines the default model (can be an alias)
  - `<model_name>`: Model configuration
    - `alias`: Optional alias to another model
    - `baseUrl`: Optional base URL for API (overrides default)
    - `apiKey.cred`: Credential name to look up in credentials file
    - `model`: Model identifier to use
    - `sendReasoningContent`: Optional. Replay assistant `reasoning_content` to the provider. Default `true`. Set `false` for providers that reject or mishandle it (e.g. DeepSeek-R1).

## Dependencies

- **rhd_chat_client**: For connecting to chat server
- **rhd_ai_client**: For making AI completion requests
- **Other plugins**: May execute tool calls made by AI

## Error Handling

- On AI request failure: adds error tag and error message
- On message conversion failure (history inconsistency): adds error tag to chat, sends nothing
- Skips chats with error tag
- Filters out error messages when building AI requests
- Tool calls in AI response are executed by other plugins

## Running the Plugin

```bash
cargo run --bin rhd_plugin_ai_completions -- \
  --server-url ws://127.0.0.1:8080/ \
  --config config.yaml \
  --plugin-id ai_completions
```

### Command-Line Arguments

- `--server-url`: WebSocket URL of the chat server
- `--config`: Path to configuration file
- `--plugin-id`: Plugin ID (defaults to "ai_completions")

## Environment Variables

Set log level:
```bash
export RUST_LOG=rhd_plugin_ai_completions=info
```

## Troubleshooting

### Plugin Not Triggering

1. Check that chat has queued messages or resolved tool calls
2. Verify chat doesn't have `ai_completions:error` tag
3. Check logs for trigger detection

### Plugin fails to connect
- Verify chat server is running
- Check WebSocket URL is correct
- Ensure network connectivity

### Configuration errors
- Verify config file exists and is valid YAML
- Check credentials file path is correct
- Ensure 'default' model is defined in config

### AI request failures
- Check API key is valid
- Verify model configuration
- Check network connectivity to AI API
- Review error messages in chat (tagged `ai_completions:error`)

## Integration with Other Plugins

This plugin emits `ai_completions:preRequest` before making AI requests. Other plugins can:
- Listen for this event
- Add messages to the queue
- Modify queued messages
- Acknowledge to allow the request to proceed

Example plugin that listens for preRequest:
```rust
client.on_custom_event(|event| async move {
    if event.event_name == "ai_completions:preRequest" {
        // Do something before AI request
        println!("AI request about to be made for chat {}", event.chat_id);
        
        // Acknowledge to allow request to proceed
        client.ack_custom_event(AckCustomEventParams {
            event_id: event.event_id,
        }).await;
    }
});
```
