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
2. **Register**: Plugin registers with a unique ID using `registerPlugin` method
3. **Recover**: Plugin calls `getPendingAcks` to fetch events it may have missed while disconnected
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, plugins list)
5. **React**: Plugin responds to events based on its logic

### Plugin Monitoring

Plugins can use built-in monitors from `rhd_chat_client`:

- **PluginsMonitor**: Tracks all registered plugins (active and inactive), manages custom event acknowledgments, provides wait logic for coordination
- **ChatMonitor**: Tracks chats and their state (messages, queue count, tags), detects trigger conditions, provides methods to query chat state

These monitors are reusable by any plugin and handle subscription management automatically.

### Custom Event Coordination

Plugins can coordinate their actions using custom events:

1. Plugin A sends a custom event (e.g., `ai_completions:preRequest`)
2. All connected plugins receive the event
3. Other plugins acknowledge the event using `ackCustomEvent`
4. Plugin A waits for all acknowledgments before proceeding
5. If a plugin doesn't acknowledge, the request times out

This mechanism ensures plugins can coordinate their actions and avoid conflicts. The `getPendingAcks` method returns all events not yet acknowledged by the calling plugin.

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
- All tool calls from the last assistant message are resolved (tool loop continuation)

**Trigger Detection:**
- Checks for queued messages in the chat
- Verifies no unresolved tool calls exist
- Skips chats with `ai_completions:error` tag

**Pre-Request Coordination:**
- Emits `ai_completions:preRequest` custom event before making AI requests
- Waits for all other plugins to acknowledge the event
- Allows other plugins to modify state or block the request

**Queued Message Processing:**
- Fetches queued messages from the chat
- Deletes each queued message from the queue
- Adds each as a regular message to maintain chat history
- Builds AI request from message history (filtering out error messages)

**Error Handling:**
- On AI request failure: adds `ai_completions:error` tag to chat
- Adds error message with role `ai_completions:error` containing error details
- Skips chats with error tag (no further processing)
- Filters out error messages when building AI requests

**Tool Call Handling:**
- If AI response contains tool calls, adds assistant message with tool calls to chat
- Other plugins execute the tools and add results as tool messages
- Plugin detects when all tool calls are resolved and continues the loop

### Future Plugin Ideas

- **Notification Plugin**: Send notifications when specific events occur
- **Logging Plugin**: Log all chat activity to external systems
- **Moderation Plugin**: Filter or flag inappropriate content
- **Translation Plugin**: Translate messages between languages
