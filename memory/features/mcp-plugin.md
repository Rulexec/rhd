# MCP Plugin Feature

## Purpose

The `rhd_plugin_mcp` plugin makes external MCP (Model Context Protocol)
servers available as chat tools. It spawns configured servers, registers
their tools on eligible chats, executes tool calls the AI makes, and pushes
results back into the conversation.

## How It Works

### Startup
- Reads `--config <path>` YAML listing MCP servers.
- Spawns and initializes ALL configured servers immediately, **best-effort**:
  a server that fails to start (spawn, initialize, or list-tools) is recorded
  as `error` with its message and excluded from routing, while the remaining
  servers keep working and the plugin stays up.
- Collects each server's tool list.
- Config parse errors (missing `env:` variables, duplicate names/ids, empty
  list, unknown fields) remain fatal — the plugin aborts startup with a clear
  error.

### Status reporting
- The plugin publishes a `status` plugin state with schema `mcpStatus:1`
  listing every configured server in config order with `ok`/`error` status
  and, for errored servers, the failure message. The UI shows it in the MCPs
  tab, which appears only while such a state exists.
- Transitions: startup failure (spawn/initialize/list-tools) and transport
  failures during tool calls mark a server `error`; a later successful call
  recovers it to `ok`.
- Tool-level `isError` results are legitimate MCP responses, **not** server
  failures — they never flip the status.
- Pushes are event-driven (on status change) and deduplicated: an unchanged
  payload is not re-pushed.

### Tool naming
- Every tool is registered prefixed with its server's name:
  `<name>:<tool>` (e.g. `filesystem:read_file`).
- The prefix identifies the owning server; routing splits at the first `:`.

### Chat gating
- **Worktree gate:**
  - With `--worktree <id>`: the plugin acts only on chats tagged
    `worktree:<id>`.
  - Without `--worktree`: the plugin acts only on chats that carry NO
    `worktree:*` tag.
- **Per-server gate (`registerOnTag`):** a server listing `registerOnTag: T`
  has its tools registered only on chats carrying tag `T`. Servers without
  the field target all otherwise-eligible chats.
- Gates combine with AND. Registration is additive: a chat that gains an
  eligible tag later receives the remaining servers' tools; tools are never
  unregistered.

### Tool execution
- When the AI calls one of the plugin's tools, the plugin executes it on the
  owning MCP server and answers with a `tool` message carrying the matching
  `toolCallId`.
- Duplicate calls (same `toolCallId` already answered) are skipped.
- MCP errors are returned as tool result content — the model decides how to
  react; the plugin keeps running.
- Calls to one server are serialized; different servers run in parallel.

## Configuration

```yaml
mcp:
  - id: fs1                      # optional, defaults to name
    name: filesystem             # tool prefix "filesystem:"
    cmd: npx
    args:
      - '-y'
      - '@modelcontextprotocol/server-filesystem'
      - env: AVAILABLE_ROOT      # resolved from plugin environment at startup
    cwd: /some/dir               # optional; default: plugin's working directory
    env:                         # optional; extra vars for the server process
      SOME_VAR: some-value
    registerOnTag: 'mcp:common'  # optional; exact chat tag gate
```

- Missing `env:` variables, duplicate `name`s, duplicate `id`s, or an empty
  `mcp:` list fail startup with a clear error.
- Unknown config fields are rejected.

## Running

```bash
rhd_plugin_mcp --server-url ws://127.0.0.1:8080/ --plugin-id mcp \
  [--worktree <workTreeId>] --config <configPath>
```

## Interaction with other plugins
- `rhd_plugin_ai_completions` drives the tool loop; this plugin answers the
  tool calls it registered. The loop continues once every call is answered.
- The plugin acknowledges all custom events it receives (handles none), so
  `ai_completions:preRequest` coordination never blocks on it.

## Error handling (user perspective)
- Server spawn/initialize failure → the plugin keeps running; the server
  appears as `error` in the MCPs tab with its message and its tools are not
  registered. Fix the config/environment and restart the plugin to bring it
  back.
- Tool-call transport failure during conversation → the AI sees an error text
  as the tool result and can retry or proceed; the server flips to `error` in
  the MCPs tab and recovers to `ok` on a later successful call.
- Caveat: MCP servers that send unsolicited notifications are not supported
  by the stdio transport.
