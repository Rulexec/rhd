# Plugins Feature

## Overview

Plugins are external applications that extend the RHD chat system's functionality. They connect to the chat server via WebSocket, register themselves, and react to events in the system.

## What Plugins Can Do

- **Monitor Chats**: Track chat activity and respond to messages
- **Automate Tasks**: Perform automated actions based on chat events
- **Integrate External Services**: Connect to AI providers, databases, or other services
- **Add Custom Logic**: Implement custom business logic for chat processing

## How Plugins Work

### Plugin Lifecycle

1. **Connect**: Plugin connects to chat server via WebSocket
2. **Register**: Plugin registers with a unique ID
3. **Recover**: Plugin fetches pending acknowledgments to catch up on missed events
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, etc.)
5. **React**: Plugin responds to events based on its logic

### Event Model

Plugins interact with the system through events:

- **Chat Events**: Message added, updated, deleted; queue message changes
- **System Events**: Chat created, updated, deleted; plugin registered, removed
- **Custom Events**: Plugins can emit and listen for custom events

### Acknowledgments

When a plugin sends a custom event, other plugins can acknowledge it. This allows coordination between plugins:

1. Plugin A sends custom event
2. Plugin B and C receive the event
3. Plugin B and C acknowledge the event
4. Plugin A waits for all acknowledgments before proceeding

This mechanism ensures plugins can coordinate their actions and avoid conflicts.

## Deploying Plugins

### Requirements

- Plugin must be a standalone binary
- Plugin must accept server URL and config file path as arguments
- Plugin must handle disconnections gracefully

### Configuration

Plugins typically use YAML configuration files with:
- Server connection details
- API keys and credentials
- Plugin-specific settings

Credentials should be stored in a separate file referenced by the main config.

### Running a Plugin

```bash
./my-plugin --server-url ws://localhost:8080/ --config plugin-config.yaml
```

### Monitoring Plugins

Use the chat server's plugin management API to:
- List active plugins
- Check plugin status
- View plugin events

## Error Handling

Plugins should handle errors gracefully:
- Log errors with context
- Retry transient failures
- Notify users of persistent failures
- Continue running even if individual operations fail

## Security Considerations

- Plugins run with the same permissions as the user who started them
- Plugins can access all chats and messages
- API keys should be stored securely
- Plugins should validate all inputs

## Example Plugins

### AI Completions Plugin

The `rhd_plugin_ai_completions` plugin monitors chats and triggers AI completions when:
- There are queued messages and no unresolved tool calls
- All tool calls from the last assistant message are resolved

It emits `ai_completions:preRequest` before making AI requests, allowing other plugins to coordinate.

### Future Plugin Ideas

- **Notification Plugin**: Send notifications when specific events occur
- **Logging Plugin**: Log all chat activity to external systems
- **Moderation Plugin**: Filter or flag inappropriate content
- **Translation Plugin**: Translate messages between languages
