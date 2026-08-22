# RHD Plugin System

## Overview

The RHD plugin system allows external binary applications to connect to the chat server, register themselves, subscribe to events, and interact with the chat system through a well-defined WebSocket API. Plugins extend the chat system's capabilities by reacting to events and performing automated actions.

## Plugin Lifecycle

Plugins follow a standard lifecycle:

1. **Connect**: Plugin connects to chat server via WebSocket
2. **Register**: Plugin calls `registerPlugin` with unique ID
3. **Get Pending Acks**: Plugin calls `getPendingAcks` to recover missed events
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, plugins list)
5. **React**: Plugin responds to events based on its logic

## Event Model

### Custom Events

Plugins can emit custom events via `sendCustomEvent`. Custom events have:
- **Event Name**: Unique identifier (e.g., `ai_completions:preRequest`)
- **Sender Plugin ID**: Automatically set to the sending plugin's ID
- **Additional Data**: JSON payload with event-specific data
- **Created At**: Timestamp of event creation

### Acknowledgments

Plugins acknowledge events via `ackCustomEvent`. This is used for coordination between plugins:
- When a plugin emits an event, other plugins can acknowledge it
- The sender can wait for all acknowledgments before proceeding
- Acknowledgments are tracked per-plugin

### Pending Acks

Events that haven't been acknowledged by the current plugin are called "pending acks". On startup, plugins must call `getPendingAcks` to handle events they may have missed while disconnected.

## Plugin Responsibilities

Each plugin should document:
- **Which events it listens for**: Chat events, custom events, etc.
- **What tags it adds to messages/chat**: For state tracking and filtering
- **What events it emits and when**: Custom events for coordination
- **Configuration format**: How to configure the plugin

## Configuration Conventions

- Plugins accept config file path as command-line argument (`--config`)
- Config files use YAML format
- Credentials should be in separate file referenced by config (`credentialsConfig`)
- Use environment variable substitution where appropriate
- Configuration should be validated on startup with clear error messages

## Creating a New Plugin

### Step-by-Step Guide

1. **Create package in `plugins/` directory**
   ```
   plugins/
     my_plugin/
       Cargo.toml
       src/
         main.rs
         config.rs
         plugin.rs
       README.md
   ```

2. **Define `Cargo.toml` with required dependencies**
   ```toml
   [package]
   name = "my_plugin"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   rhd_chat_client = { path = "../../packages/rhd_chat_client" }
   rhd_chat_api = { path = "../../packages/rhd_chat_api" }
   tokio = { version = "1", features = ["full"] }
   serde = { version = "1", features = ["derive"] }
   serde_yaml = "0.9"
   tracing = "0.1"
   tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   clap = { version = "4", features = ["derive"] }
   thiserror = "1"
   ```

3. **Implement configuration loading**
   - Define config structures with `serde::Deserialize`
   - Implement `load_config()` function
   - Validate configuration on load
   - Handle credentials separately

4. **Implement main plugin logic**
   - Connect to chat server
   - Register as plugin
   - Get and process pending acks
   - Subscribe to relevant events
   - Implement event handlers
   - Keep plugin running

5. **Create README documenting plugin behavior**
   - Trigger conditions
   - Events emitted
   - Tags added
   - Configuration format
   - Dependencies

## Plugin README Template

```markdown
# My Plugin

## Overview
Brief description of what this plugin does.

## Trigger Conditions
When does this plugin activate?

## Events Emitted
List of custom events this plugin emits:
- `event_name`: Description and payload format

## Tags Added
List of tags this plugin adds:
- `tag_name`: When and why this tag is added

## Configuration Format
```yaml
# Example configuration
```

## Dependencies
- **rhd_chat_client**: For connecting to chat server
- **Other plugins**: List any dependencies on other plugins

## Error Handling
How does this plugin handle errors?
```

## Best Practices

1. **Error Handling**: Always handle errors gracefully and log them with `tracing`
2. **Logging**: Use structured logging with appropriate log levels
3. **Configuration Validation**: Validate all configuration on startup
4. **Event Coordination**: Use custom events and acknowledgments for coordination
5. **State Management**: Use tags to track state and prevent duplicate processing
6. **Graceful Shutdown**: Handle shutdown signals gracefully

## Examples

See `plugins/rhd_plugin_ai_completions/` for a complete example implementation.
