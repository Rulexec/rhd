# rhd

Rust-based chat system with plugin support. The project consists of a WebSocket server for chat storage, a CLI tool for manual testing and operations, and a plugin system for extending functionality.

## Architecture

- **rhd_chat_server**: WebSocket server for chat storage and message management
- **rhd_chat_client**: Client library for communicating with the server
- **rhd_app**: CLI tool for interacting with the chat server
- **rhd_plugin_ai_completions**: Plugin for AI-powered message completions

## Install

```bash
cargo build --release
```

Binaries:
- `target/release/rhd` - CLI tool
- `target/release/rhd_chat_server` - Chat server

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

## Packages

- **rhd_util**: Shared utilities
- **rhd_db**: SQLite database layer
- **rhd_chat_api**: API types for chat WebSocket protocol
- **rhd_chat_server**: WebSocket server for chat storage
- **rhd_chat_client**: Chat client library
- **rhd_mcp_client**: MCP protocol client
- **rhd_mock_ai_provider**: Mock AI provider for testing
- **rhd_app**: CLI tool for chat server interaction
- **rhd_plugin_ai_completions**: AI completions plugin
- **rhd_plugin_choice**: Choice tool plugin

## Plugins

Plugins extend the chat system functionality. See `plugins/README.md` for plugin development documentation.

### rhd_plugin_ai_completions

The AI completions plugin processes queued messages and generates AI responses. It monitors the message queue and automatically adds assistant messages to chats.

### rhd_plugin_choice

The choice plugin provides the `rhd_choice` tool to every chat so the assistant can ask the user to choose between options. The frontend renders the choices as buttons plus a free-text input and answers the tool call on the user's behalf; an unanswered choice pauses the assistant's tool loop until the user decides.

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Run Server

```bash
cargo run --bin rhd_chat_server
```

### Run CLI

```bash
cargo run --bin rhd -- <command>
```
