# Phase 3: Integration Testing and Documentation

## Overview

This phase verifies the CLI works correctly with `rhd_chat_server` and `rhd_plugin_ai_completions`, and documents the usage. The goal is to ensure the manual testing workflow is functional and well-documented.

**Scope:**
- Test all CLI commands against running server
- Verify plugin processing workflow
- Update README with CLI usage
- Create feature documentation

**Out of scope:**
- Memory cleanup (Phase 4)
- Code cleanup in other packages

## Files to Modify

### 1. `README.md`

**Add new section:**

```markdown
## CLI Usage

The `rhd` CLI provides command-line access to `rhd_chat_server` for manual testing and operations.

### Starting the Server

Before using the CLI, start the chat server:

```bash
cargo run --bin rhd_chat_server
```

The server will listen on `ws://127.0.0.1:8080/` by default.

### CLI Commands

#### List Chats

```bash
rhd chats list
```

Output: JSON array of chat objects.

#### View Messages

```bash
# Show only last message
rhd messages <chat_id>

# Show all messages
rhd messages <chat_id> --all
```

Output: JSON array of message objects.

#### Queue Operations

```bash
# List queued messages
rhd queue list <chat_id>

# Add message to queue
rhd queue add <chat_id> "Your message content"
```

#### Create Chat

```bash
rhd create-chat "Chat Title"
```

Output: JSON object with created chat.

#### Plugin Operations

```bash
# List plugins
rhd plugins list

# Remove plugin
rhd plugins remove <plugin_id>
```

### Manual Testing Workflow

To test `rhd_plugin_ai_completions`:

1. **Start the server:**
   ```bash
   cargo run --bin rhd_chat_server
   ```

2. **Create a chat:**
   ```bash
   rhd create-chat "Test Chat"
   ```
   Note the chat ID from the output.

3. **Queue a message:**
   ```bash
   rhd queue add <chat_id> "Hello, AI!"
   ```

4. **Verify plugin processing:**
   Check server logs to see if `rhd_plugin_ai_completions` processed the message.

5. **Check for assistant message:**
   ```bash
   rhd messages <chat_id> --all
   ```
   Look for an assistant message in the output.
```

### 2. `memory/features/cli.md`

**Create new file:**

```markdown
# CLI Feature

## Overview

The RHD CLI provides command-line access to the chat server for manual testing and operations. It enables users to interact with chats, messages, queues, and plugins without a graphical interface.

## Commands

### Chat Management

- **List chats**: `rhd chats list`
  - Returns all chats in JSON format
  - Each chat includes: id, title, created_at, tags

- **Create chat**: `rhd create-chat <title>`
  - Creates a new chat with the specified title
  - Returns the created chat object

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
```

## Testing Checklist

### Basic Operations

- [ ] `rhd chats list` returns empty list on fresh server
- [ ] `rhd create-chat "Test"` creates a chat and returns chat object
- [ ] `rhd chats list` returns the created chat
- [ ] `rhd messages <chat_id>` returns empty or last message
- [ ] `rhd messages <chat_id> --all` returns all messages

### Queue Operations

- [ ] `rhd queue list <chat_id>` returns empty list initially
- [ ] `rhd queue add <chat_id> "test"` adds message to queue
- [ ] `rhd queue list <chat_id>` shows the queued message
- [ ] After plugin processing, queue is empty

### Plugin Operations

- [ ] `rhd plugins list` shows registered plugins
- [ ] `rhd plugins remove <plugin_id>` removes the plugin
- [ ] `rhd plugins list` no longer shows removed plugin

### Integration with rhd_plugin_ai_completions

- [ ] Create chat
- [ ] Queue a user message
- [ ] Verify plugin processes the message (check server logs)
- [ ] Verify assistant message is added to chat
- [ ] Use `rhd messages --all` to see both user and assistant messages

## Implementation Notes

1. **Server requirement:** All tests require `rhd_chat_server` to be running
2. **Plugin requirement:** Testing plugin workflow requires `rhd_plugin_ai_completions` to be registered
3. **JSON parsing:** Use `jq` or similar tools to parse JSON output in scripts
4. **Error handling:** CLI outputs errors to stderr with non-zero exit code

## Dependencies

- Phase 2 must be complete (CLI implemented)
- Requires running `rhd_chat_server`
- Requires `rhd_plugin_ai_completions` for plugin testing

## Success Criteria

- [ ] All CLI commands tested and working
- [ ] README updated with CLI usage section
- [ ] Feature documentation created in `memory/features/cli.md`
- [ ] Manual testing workflow verified end-to-end
- [ ] Plugin processing workflow documented and tested
