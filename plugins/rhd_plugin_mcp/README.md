# MCP Plugin

## Overview
Bridges external MCP (Model Context Protocol) servers into RHD chats.
Spawns configured servers at startup (best-effort per server), registers
their tools on eligible chats prefixed `<name>:`, executes tool calls,
pushes results back as `tool`-role messages, and exposes the per-server
health fleet as plugin state (`mcpStatus:1`).

## CLI Usage
    rhd_plugin_mcp --server-url ws://127.0.0.1:8080/ --plugin-id mcp \
      [--worktree <workTreeId>] --config <configPath>

## Trigger Conditions (chat gating)
- With `--worktree W`: only chats tagged exactly `worktree:W`.
- Without `--worktree`: only chats carrying NO `worktree:*` tag.
- Per server: if `registerOnTag` is set, the server's tools register only
  on chats carrying that exact tag (AND with the worktree rule).
- Registration is additive: a chat that gains an eligible tag later gets the
  remaining servers' tools registered; tools are never unregistered.

## Configuration Format
```yaml
mcp:
  - id: fs1                      # optional, defaults to name
    name: filesystem             # tool prefix: "filesystem:"
    cmd: npx
    args:
      - '-y'
      - '@modelcontextprotocol/server-filesystem'
      - env: AVAILABLE_ROOT      # resolved from plugin environment at startup
    cwd: /abs/or/relative/path   # optional; default: plugin's working directory
    env:                         # optional; extra vars for the server process
      SOME_VAR: some-value
    registerOnTag: 'mcp:common'  # optional; exact chat tag gate
```
Missing `env:` variables fail startup with a clear error. Duplicate `name`
or `id` values are rejected.

## Status reporting
The plugin publishes its server fleet as plugin state:

- **key**: `status` · **format**: `json` · **schema**: `mcpStatus:1`
- **content**: `{"mcp":[{"id","name","status":"ok"|"error","error"?}]}`
  in config order; `error` is present only for errored servers
  (startup / broken-protocol / crash-on-call message).

```json
{
  "mcp": [
    { "id": "fs", "name": "filesystem", "status": "ok" },
    { "id": "s", "name": "search", "status": "error",
      "error": "spawn/initialize failed: No such file or directory (os error 2)" }
  ]
}
```

Pushes are event-driven (no polling): after startup (initial state), when a
tool call hits a transport/protocol failure on a server, and when that
server later completes a successful call (recovery). A push is skipped when
the serialized payload is identical to the last successful one (dedup).
Tool-level `isError` responses are legitimate MCP results and do NOT flip
server status. Steady-state push failures are logged and retried on the
next change; only the initial push can abort the plugin.

## Events Emitted
None (state updates go through `updatePluginState`, see above).

## Tags Added
None (reads chat tags; does not mutate them).

## Tags Consumed
- `worktree:<id>` — worktree gating (see Trigger Conditions).
- value of `registerOnTag` per server (e.g. `mcp:common`).

## Tool Call Handling
- Subscribes to `assistantMessageWithToolCalls` for its registered prefixed
  names; routes `{name}:{tool}` to the owning server; answers every call with
  a `tool`-role message carrying `toolCallId`.
- Duplicate guard: skips calls already answered (tool message with same id).
- MCP/transport errors are returned as tool content, not plugin crashes.

## Dependencies
- **rhd_chat_client / rhd_chat_api**: chat server interaction.
- **rhd_mcp_client**: MCP stdio JSON-RPC client.
- **rhd_plugin_ai_completions**: executes the tool loop; this plugin answers
  the tool calls it registered.

## Error Handling
- Config load/parse errors are fatal (nothing to run without a valid fleet).
- Startup is best-effort per server: a server that fails to spawn/initialize
  is reported as `"error"` in the `mcpStatus:1` state and excluded from
  tool routing; healthy servers keep working and the plugin keeps running.
- Runtime: per-call errors surface as tool results; plugin keeps running.
- Caveat: the stdio transport assumes one response line per request; MCP
  servers that emit unsolicited notifications are not supported.
