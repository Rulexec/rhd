# rhd_plugin_system_prompt

A plugin for the RHD chat system that automatically injects system prompts into chats based on chat tags.

## Overview

This plugin monitors chats for tags like `systemPrompt:warhammer` and adds corresponding system prompt messages from configured markdown files. It coordinates with the AI completions plugin to ensure system prompts are in place before AI requests proceed.

## Features

- **Automatic System Prompt Injection**: Automatically adds system prompts to chats based on tags
- **Multiple Prompts per Chat**: Supports multiple system prompts in a single chat
- **Duplicate Prevention**: Prevents adding the same system prompt twice
- **Race Condition Handling**: Safely handles chats with unfinished assistant messages
- **Event Coordination**: Coordinates with AI completions plugin via `ai_completions:preRequest` events

## Installation

Build the plugin:

```bash
cargo build --release -p rhd_plugin_system_prompt
```

The binary will be available at `target/release/rhd_plugin_system_prompt`.

## Configuration

Create a YAML configuration file:

```yaml
systemPrompts:
  warhammer: ./prompts/warhammer.md
  jokeTeller: ./prompts/jokes.md
```

### Configuration Options

- `systemPrompts`: A map of prompt names to file paths
  - Keys are prompt names (used in tags as `systemPrompt:<name>`)
  - Values are paths to markdown files containing the prompt content
  - Relative paths are resolved against the config file's directory
  - Absolute paths are used as-is

## Usage

Run the plugin:

```bash
./rhd_plugin_system_prompt \
  --server-url ws://localhost:8080/ \
  --config config.yaml \
  --plugin-id system_prompt
```

### Command-Line Arguments

- `--server-url`: WebSocket URL of the chat server (required)
- `--config`: Path to configuration file (required)
- `--plugin-id`: Plugin ID (optional, defaults to `system_prompt`)

## How It Works

1. **Startup**:
   - Loads configuration and validates all prompt files exist
   - Connects to the chat server and registers as a plugin
   - Subscribes to all chats
   - Performs startup reconciliation to inject prompts for existing chats

2. **Main Loop**:
   - Periodically checks all monitored chats for missing system prompts
   - Injects any missing prompts

3. **Event Coordination**:
   - Listens for `ai_completions:preRequest` events
   - If a chat needs a system prompt, injects it before acknowledging the event
   - Ensures system prompts are in place before AI requests proceed

## Interaction with AI Completions Tags

The plugin never injects system prompts into a chat while the chat is locked
by the AI completions plugin, i.e. while its tags contain either:

- `ai_completions:running` — an AI request is actively in progress
  (including between tool-loop iterations)
- `ai_completions:error` — the chat is parked after a failure

**Effect:** injection happens only when the chat awaits its next queued user
message, or when the chat is new. If a `systemPrompt:<name>` tag is added to a
locked chat, the prompt is injected as soon as the lock tag is removed:

- after `running` is removed (request finished) — within one main-loop tick
- after `error` is removed by an operator — on the next main-loop tick

The `ai_completions:preRequest` event is always acknowledged, even when
injection is skipped, so the AI completions plugin is never blocked.

## Tag Format

Chats should be tagged with `systemPrompt:<name>` where `<name>` matches a key in the configuration.

Example tags:
- `systemPrompt:warhammer`
- `systemPrompt:jokeTeller`

## System Prompt Messages

When a system prompt is injected, a message is added to the chat with:
- `role: "system"`
- `content: <markdown file content>`
- `tags: ["systemPrompt:<name>"]`
- `is_finished: true`
- `is_streaming: false`

## Error Handling

- **Missing Prompt Files**: Plugin fails to start if any configured prompt file is missing
- **Injection Errors**: Logged but don't stop the main loop
- **Event Acknowledgment**: Always acknowledges events, even if injection fails

## Troubleshooting

### Plugin fails to start with "failed to read prompt file"

Ensure all prompt files exist and are readable. Check the paths in your configuration file.

### System prompts not being injected

1. Verify the chat has the correct tag (e.g., `systemPrompt:warhammer`)
2. Check that the prompt name matches a key in the configuration
3. Ensure the chat doesn't have an unfinished assistant message
4. Check that the chat does not have an `ai_completions:running` or
   `ai_completions:error` tag — injection is deferred until such a tag is removed

### AI requests proceeding without system prompts

Check the plugin logs for errors during event handling. The plugin should log when it receives `ai_completions:preRequest` events and when it injects system prompts.

## Development

### Running Tests

```bash
cargo test -p rhd_plugin_system_prompt
```

### Code Structure

- `src/main.rs`: Entry point with CLI argument parsing
- `src/lib.rs`: Module exports
- `src/config.rs`: Configuration loading and validation
- `src/plugin.rs`: Core plugin lifecycle and main loop
- `src/system_prompt.rs`: System prompt detection and injection logic

## License

This project is part of the RHD workspace.
