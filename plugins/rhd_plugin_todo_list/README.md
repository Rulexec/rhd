# rhd_plugin_todo_list

A plugin for managing task tracking lists in RHD chat conversations.

## Overview

The `rhd_plugin_todo_list` plugin provides a structured way to track multi-step tasks within chat conversations. It automatically injects a system message with the tool contract when a new chat is created and handles the `rhd_set_todo_list` tool calls to maintain the todo list state.

## Features

- **Automatic Initialization**: Injects a system message with the tool contract and registers the `rhd_set_todo_list` tool when a new chat is created
- **Todo List Management**: Parses markdown checklists and maintains per-chat todo list state
- **AI Context Injection**: Injects the current todo list state before AI requests via the `ai_completions:preRequest` event
- **Error Handling**: Provides clear error messages with examples when the todo list format is invalid

## How It Works

### Chat Initialization

When a new chat is created, the plugin:
1. Checks if the contract system message already exists (tagged with `todo_list:contract`)
2. If not, adds a system message with the tool contract
3. Registers the `rhd_set_todo_list` tool for the chat

### Tool Call Handling

When the AI calls `rhd_set_todo_list`:
1. The plugin receives the tool call via the `assistantMessageWithToolCalls` event
2. Parses the `todos` parameter as a markdown checklist
3. If valid, stores the todo list and returns a success message
4. If invalid, returns an error message with the correct format example

### AI Context Injection

Before each AI request:
1. The plugin receives the `ai_completions:preRequest` event
2. Retrieves the current todo list for the chat
3. Injects a system message with the todo list (or an empty template if no items exist)
4. Acknowledges the event to allow the AI request to proceed

## Todo List Format

The todo list uses a markdown checklist format:

```markdown
[ ] Pending task
[-] In progress task
[x] Completed task
[!] Discarded task
```

### Checkbox States

| Syntax | Status | Description |
|--------|--------|-------------|
| `[ ]` | Pending | Task not yet started |
| `[-]` | In Progress | Task currently being worked on |
| `[x]` | Completed | Task finished |
| `[!]` | Discarded | Task no longer needed |

## Configuration

The plugin is configured via command-line arguments:

```bash
rhd_plugin_todo_list --server-url ws://localhost:8080/ [--plugin-id todo_list]
```

### Arguments

- `--server-url` (required): WebSocket URL of the chat server
- `--plugin-id` (optional): Plugin identifier (default: `todo_list`)

## Architecture

### Components

- **parser**: Parses markdown checklists into structured todo items
- **todo_store**: Thread-safe per-chat todo list storage
- **templates**: Loads and renders templates for system messages and error responses
- **tool_handler**: Handles `rhd_set_todo_list` tool calls
- **plugin**: Main plugin lifecycle and event handling

### Event Flow

```
Chat Created
    ↓
Plugin detects new chat
    ↓
Add contract system message (tagged: todo_list:contract)
    ↓
Register rhd_set_todo_list tool
    ↓
AI calls rhd_set_todo_list
    ↓
Plugin receives assistantMessageWithToolCalls event
    ↓
Parse and validate todo list
    ↓
Store todo list / Return error
    ↓
AI requests completion
    ↓
Plugin receives ai_completions:preRequest event
    ↓
Inject current todo list as system message
    ↓
Acknowledge event
```

## Templates

The plugin uses the following templates:

- `templates/mcp_internal/rhd_set_todo_list/contract.md`: Tool contract and usage instructions
- `templates/mcp_internal/rhd_set_todo_list/tool_definition.json`: Tool definition JSON
- `templates/mcp_internal/rhd_set_todo_list/tool_error_invalid_format.md`: Error message for invalid format
- `templates/environment/todo_list_with_items.md`: Template for rendering todo list with items
- `templates/environment/todo_list_empty.md`: Template for empty todo list

## Testing

Run the test suite:

```bash
cargo test --package rhd_plugin_todo_list
```

The test suite includes:
- Unit tests for parser, todo store, templates, and tool handler
- Integration tests for end-to-end functionality

## License

Part of the RHD project.
