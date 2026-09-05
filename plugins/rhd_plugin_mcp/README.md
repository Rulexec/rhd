# MCP Plugin

## Overview
Bridges external MCP (Model Context Protocol) servers into RHD chats.
Spawns configured servers at startup, registers their tools on eligible
chats prefixed `<name>:`, executes tool calls, and pushes results back
as `tool`-role messages.

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

## Events Emitted
None.

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
- Startup: any server spawn/initialize failure aborts the plugin (fail-fast).
- Runtime: per-call errors surface as tool results; plugin keeps running.
- Caveat: the stdio transport assumes one response line per request; MCP
  servers that emit unsolicited notifications are not supported.
