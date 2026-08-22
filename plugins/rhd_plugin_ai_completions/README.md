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
- Chat has no unresolved tool calls
- Chat does not have `ai_completions:error` tag

### Tool Loop Continuation Mode

The plugin triggers when:
- Last assistant message has tool calls
- All tool calls have corresponding tool result messages
- Chat does not have `ai_completions:error` tag

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

Added to chat when AI request fails.

**When**: AI completion request returns error (timeout, API error, etc.)

**Effect**: Plugin skips chats with this tag (no further processing).

## Messages Added

### Error Messages

When AI request fails, plugin adds message with:
- **Role**: `ai_completions:error`
- **Content**: Error details (error message, timeout info, etc.)

**Filtering**: These messages are filtered out when building AI requests (only `user`, `assistant`, `system`, `tool` roles are sent to AI).

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

## Dependencies

- **rhd_chat_client**: For connecting to chat server
- **rhd_ai_client**: For making AI completion requests
- **Other plugins**: May execute tool calls made by AI

## Error Handling

- On AI request failure: adds error tag and error message
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
- Review error messages in chat (role: `ai_completions:error`)

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
