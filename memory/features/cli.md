# CLI Feature

## Overview

The RHD CLI provides command-line access to the chat server for manual testing and operations. It enables users to interact with chats, messages, queues, and plugins without a graphical interface.

## Commands

### Chat Management

- **List chats**: `rhd chats list`
  - Returns all chats in JSON format
  - Each chat includes: id, title, created_at, tags

- **Create chat**: `rhd create-chat <title> [--tags <tag1> <tag2> ...]`
  - Creates a new chat with the specified title
  - Optional `--tags` sets initial tags on the new chat (space-separated)
  - Returns the created chat object

- **Add tags**: `rhd chats add-tag <chat_id> <tag1> [<tag2> ...]`
  - Adds one or more tags to an existing chat
  - Prints a success message; backend validates gracefully (adding an existing tag is a no-op)

- **Remove tags**: `rhd chats remove-tag <chat_id> <tag1> [<tag2> ...]`
  - Removes one or more tags from a chat
  - Prints a success message; removing a non-existent tag is handled gracefully

### Message Operations

- **View messages**: `rhd messages <chat_id> [--all]`
  - Default: shows only the last message
  - With `--all`: shows all messages in the chat
  - Messages include: id, role, content, created_at, model

### Queue Management

- **List queued messages**: `rhd queue list <chat_id>`
  - Shows all messages waiting to be processed
  - Each queued message includes: id, chat_id, content, created_at

- **Add to queue**: `rhd queue add <chat_id> <content>`
  - Adds a new message to the chat's processing queue
  - Returns the queued message ID

### Plugin Management

- **List plugins**: `rhd plugins list`
  - Shows all registered plugins
  - Each plugin includes: id, name, status

- **Remove plugin**: `rhd plugins remove <plugin_id>`
  - Unregisters a plugin from the server

## Manual Testing Workflow

The CLI enables a complete manual testing workflow for plugins:

1. **Setup**: Start `rhd_chat_server`
2. **Create chat**: Use `rhd create-chat` to create a test chat
3. **Queue message**: Use `rhd queue add` to queue a user message
4. **Verify processing**: Check server logs for plugin activity
5. **Check results**: Use `rhd messages --all` to view assistant responses

This workflow is particularly useful for testing `rhd_plugin_ai_completions` and verifying that AI responses are correctly added to chats.

## Output Format

All commands output JSON for easy parsing and scripting:

- Lists are output as JSON arrays
- Single objects are output as JSON objects
- Success messages are plain text
- Errors are output to stderr

## Configuration

Currently, the CLI connects to `ws://127.0.0.1:8080/` by default. Future versions may support:
- Custom server URLs via environment variable
- Configuration file for default settings
- Connection pooling for better performance
