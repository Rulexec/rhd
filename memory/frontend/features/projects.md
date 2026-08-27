# Projects

## Purpose
Projects bundle MCP server configurations and system prompts into reusable contexts that can be attached to chats. When a project is attached to a chat, its MCP servers are spawned (if not already running) and its system prompt is injected into the conversation, enabling the AI to use project-specific tools and follow project-specific instructions.

## What Is a Project?
A project is a directory containing:
- `mcp.yaml` — MCP server references (which tools the project provides)
- `systemPrompt.md` (optional) — Instructions injected at the start of conversations

Project name is derived from the directory name.

## Project Structure

```
projects/
├── my-project/
│   ├── mcp.yaml         # MCP server configurations
│   └── systemPrompt.md  # Optional system prompt
└── another-project/
    └── mcp.yaml
```

### mcp.yaml Format
```yaml
- name: fs                    # Reference to mcp/fs.yaml (or mcp/fs/mcp.yaml)
  id: fs1                     # Optional, defaults to name if not specified
  env:
    AVAILABLE_ROOT: /path     # Environment variable overrides
  args: ["--extra-arg"]       # Optional argument overrides
- name: flags                 # Built-in tools (not prefixed with ID)
```

The `name` field references an MCP config in the `mcp/` directory. The `id` field (optional) is used for tool namespacing — if not specified, defaults to `name`.

### systemPrompt.md
Plain markdown text injected as a system message at the start of the conversation. Only injected once per project per chat (tracked in database to avoid duplicates).

## How It Works

### Attaching Projects to Chats
1. User opens a chat
2. User selects a project from the projects panel
3. System spawns the project's MCP servers (if not already running)
4. System records the attachment in the database
5. On the first message sent, the system prompt is injected
6. MCP tools become available to the AI during the conversation

### MCP Server Lifecycle
- **Spawned**: When project is first attached to any chat
- **Cached**: Reused across multiple chats and attachments
- **Killed**: Only on daemon shutdown or `rhd reload` (when config changes)
- **Not killed**: When project is detached from a chat (servers stay alive for reuse)

### System Prompt Injection
- Injected as a system message at the beginning of the conversation
- Happens on the **first message** after project attachment
- Tracked per-project per-chat in database
- Not re-injected on subsequent messages (avoid duplicates)
- If project is detached and re-attached, system prompt is injected again

### Tool Namespacing
All MCP tools from projects are namespaced by their MCP ID:
- Tool names sent to AI: `{mcp_id}/{tool_name}` (e.g., `fs1/read_file`)
- Built-in tools (e.g., `rhd_set_flag`) are **not** namespaced
- When AI calls a tool, the system parses the prefix to route to the correct MCP server

### Conflict Detection
When attaching a project to a chat:
- System checks for duplicate MCP IDs across all attached projects
- If duplicate IDs found, attachment is rejected with error
- Error lists the conflicting MCP IDs
- User must detach one of the conflicting projects or rename MCP IDs

## WebSocket Protocol

### Requests
- `listProjects` — get all available projects
- `getProjectMcpStatus` — get MCP status for a project
- `attachProject` — attach project to chat
- `detachProject` — detach project from chat
- `getChatProjects` — get projects attached to current chat

### Events
- `projectAttached` — project attached to chat (chatId, projectName)
- `projectDetached` — project detached from chat (chatId, projectName)
- `projectMcpStatusChanged` — MCP status changed (projectName, mcpName, status, error?)

## Error Handling

### MCP Server Spawn Failure
- Individual MCP failure doesn't block other MCPs
- Failed status includes error message for UI display
- User can retry by detaching and re-attaching project

### Duplicate MCP IDs
- Attachment rejected with list of conflicting IDs
- User must resolve conflict before attaching

### MCP Server Crash
- Status changes to "failed"
- UI shows error indicator
- User can retry by detaching and re-attaching
