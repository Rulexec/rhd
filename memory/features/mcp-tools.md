# MCP Tools

## Purpose
Enable `aiChat` scenario steps to use external tools via MCP (Model Context Protocol) servers and built-in tool implementations. Tools allow AI to interact with external systems (filesystem, flags, etc.) during scenario execution.

## MCP ID Namespacing
All MCP tools are namespaced by their MCP ID when sent to the AI. Tool names use the format `{mcp_id}/{tool_name}`.

- **MCP ID**: Each MCP configuration has an `id` field. If not specified, defaults to the `name` field.
- **Tool prefixing**: All MCP tools are prefixed with `{mcp_id}/` before being sent to the AI.
- **Built-in tools**: Internal tools like `rhd_set_flag` are NOT prefixed.
- **Routing**: When the AI calls a tool, the system parses the prefix to route the call to the correct MCP server.
- **Validation**: Within a single project or scenario step, duplicate MCP IDs are not allowed.

## How It Works

### Behavior Modes
- **Single-shot** (default): `aiChat` without `mcp` field — send prompt, receive single response
- **Tool loop**: `aiChat` with `mcp` field — loop: send request → model returns tool calls → execute tools → feed results back → repeat until model stops calling tools

### MCP Server Lifecycle
- Spawned on first use (when step with given MCP config runs)
- Cached at daemon level — reused across scenario executions
- Keyed by `(cmd, args, cwd)` — same config reuses same server
- Killed on daemon shutdown or during `rhd reload` when config changes
- Uses stdio transport (JSON-RPC 2.0 over line-delimited JSON)

**Reload behavior:** When `rhd reload` is executed:
- MCP servers whose config (cmd/args/cwd) changed are stopped and will be respawned on next use
- MCP servers removed from config are stopped (PID logged for manual kill if needed)
- New MCP configs are loaded but servers are spawned lazily on first use

### Built-in Tools
- `rhd_set_flag` — set a named flag with boolean value
- Flags bound to `aiChat` step name: `ai1.flag_example`, `ai2.flag_example` are independent
- Stored in execution context, accessible to all subsequent steps

### Skip Conditions
- `skip: <aiStepName>.flag_<flagName>` — if flag is `true`, skip step
- Applies to all step types except `output`
- Enables cross-step conditional execution based on AI decisions

### Tool Call Loop
1. Send request to AI with tool definitions
2. If model returns `tool_calls`, execute each tool
3. Feed tool results back to AI
4. Repeat until model returns `finish_reason: "stop"`
5. Max iterations guard (configurable per step, default 20, `"inf"` for unlimited)

### Tool Execution Routing
- Built-in tools (names starting with `rhd_`) handled internally
- External tools routed to appropriate MCP server based on tool name
- Tool errors returned as tool result content (model decides how to handle)

### Logging
- Each tool call logged: name, arguments
- Each tool result logged: content (full, no truncation)
- Available tools logged before AI request

## Configuration

### MCP Server Config
MCP servers defined in `mcp/<name>/mcp.yaml`:
```yaml
id: fs1  # optional, defaults to name if not specified
name: "filesystem"
cmd: "npx"
args: ["-y", "@modelcontextprotocol/server-filesystem", "$AVAILABLE_ROOT"]
cwd: null  # optional working directory
```

### Scenario Usage
```yaml
actions:
  - type: aiChat
    name: ai1
    model: gpt-4
    maxToolIterations: 20  # optional, default 20, "inf" for unlimited
    mcp:
      - name: fs  # reference to mcp/fs/mcp.yaml
        id: fs1  # optional, defaults to name
        env:
          AVAILABLE_ROOT: /home/user/project
        args: ["--extra-arg"]  # optional override
      - name: flags  # built-in tools (not prefixed)
    systemPrompt: "..."
    message: "..."

  - type: runCommand
    name: build
    cmd: make
    skip: ai1.flag_skip_build  # skip if ai1 set flag_skip_build to true
```

## Error Handling
- MCP tool errors → returned as tool result content (model decides how to handle)
- MCP server crash → error returned as tool result
- Max iterations exceeded → scenario fails with error
- Tool name collisions → first match wins, warning logged

### Tool Call Error UI
When a tool call fails (MCP returns `is_error: true`):
- Frontend displays red X icon (✗) instead of green checkmark (✓)
- Tool call card gets red border styling
- Error message shown in result section
- Status set to `'failed'` (vs `'completed'` for success)
- Backend propagates `is_error` flag via `ToolCallCompleted` event
- UI already has error state logic; backend now properly signals failures

## MCP Testing Infrastructure

### Mock MCP Server
- `packages/rhd_test/src/mock_mcp_server.rs` - stdio JSON-RPC 2.0 server
- Supports `tools/list` and `tools/call` methods
- Returns configurable tool definitions and responses
- CLI: `rhd_test mcp-server`

### Test Project
- `test_e2e/projects/test-project-mcp/` - project with MCP configuration
- `mcp.yaml` - references mock MCP server
- `systemPrompt.md` - instructs AI to use MCP tools

### Frontend E2E Test
- `frontend/src/tests/e2e/chat-mcp-tools.test.ts` - tests chat with MCP tools
- Verifies: MCP connects, tool calls execute, streaming works

## Key Files
- MCP client crate: `packages/rhd_mcp_client/`
- Built-in tools: `packages/rhd_mcp_client/src/builtin.rs`
- MCP server cache: `packages/rhd_app/src/mcp_cache.rs`
- Tool execution: `packages/rhd_app/src/scenario/ai_chat.rs`
- MCP config loading: `packages/rhd_app/src/scenario/loader.rs`
- Chat tool loop: `packages/rhd_chat/src/tools.rs`
- Mock MCP server: `packages/rhd_test/src/mock_mcp_server.rs`
