# Phase 7: Memory base update (replace stale MCP feature doc)

## Overview

Remove the stale `memory/features/mcp-tools.md` (it describes the removed scenario-level MCP integration with `{mcp_id}/` prefixes and scenario flags) and replace it with product-view documentation of the new `rhd_plugin_mcp` feature. Update the knowledge-base index and the plugins feature doc accordingly.

**Scope:**
- In: `memory/` files only.
- Out: any code changes.

**Depends on:** Phases 2–5 (documents shipped behavior).

## Files to Modify/Create/Delete

### 1. Delete `memory/features/mcp-tools.md`

```bash
git rm memory/features/mcp-tools.md
```

### 2. Create `memory/features/mcp-plugin.md`

Product-view feature memory (no crate APIs, no schemas — per `memory/MEMORY.md` layering). Full content to write:

```markdown
# MCP Plugin Feature

## Purpose

The `rhd_plugin_mcp` plugin makes external MCP (Model Context Protocol)
servers available as chat tools. It spawns configured servers, registers
their tools on eligible chats, executes tool calls the AI makes, and pushes
results back into the conversation.

## How It Works

### Startup
- Reads `--config <path>` YAML listing MCP servers.
- Spawns and initializes ALL configured servers immediately; any failure
  aborts plugin startup with a clear error.
- Collects each server's tool list.

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
- Server spawn/initialize failure → plugin exits at startup; fix config or
  environment and restart.
- Tool failure during conversation → the AI sees an error text as the tool
  result and can retry or proceed.
- Caveat: MCP servers that send unsolicited notifications are not supported
  by the stdio transport.
```

### 3. Modify `memory/MEMORY.md`

**Modification A — index row (line 132):**

Before:
```markdown
| [features/mcp-tools.md](features/mcp-tools.md) | Understanding MCP tool integration, built-in tools, flags, skip conditions, tool call error UI |
```

After:
```markdown
| [features/mcp-plugin.md](features/mcp-plugin.md) | Understanding the MCP plugin: spawning MCP servers, `<name>:` tool prefixes, worktree/`registerOnTag` chat gating, tool-call execution and result push |
```

**Modification B — plugins index row (line 137):** append to the "when to read" description: `..., mcp plugin (MCP servers as chat tools)`.

### 4. Modify `memory/features/plugins.md`

**Addition — new subsection after "### Todo List Plugin"** (before "### Future Plugin Ideas"):

```markdown
### MCP Plugin

The `rhd_plugin_mcp` plugin exposes external MCP servers as chat tools.

**Configuration:**
- YAML `mcp:` list: `id` (defaults to `name`), `name`, `cmd`, `args`
  (literals or `env: VAR` resolved from the plugin environment), `cwd`
  (default: plugin's working directory), `env` map for the server process,
  optional `registerOnTag`.

**Behavior:**
- Spawns all configured servers at startup (fail-fast).
- Registers tools on eligible chats prefixed `<name>:` (e.g. `filesystem:read_file`).
- Gating: with `--worktree W`, only chats tagged `worktree:W`; without it,
  only chats with no `worktree:*` tag. `registerOnTag` additionally requires
  that exact chat tag. Registration is additive per (chat, server).
- Subscribes to its tool calls, executes them on the owning server
  (serialized per server), and answers with `tool`-role messages carrying
  `toolCallId`; duplicate calls are skipped; MCP errors become tool content.
- Acknowledges all custom events (handles none).

See [features/mcp-plugin.md](features/mcp-plugin.md) for the product view.
```

### 5. Optional cleanup — `memory/scenarios.md`

The scenario-level MCP sections (`mcp/<name>/mcp.yaml`, `aiChat.mcp`, `rhd_set_flag`, skip-by-flag) describe the removed scenario integration (`rhd_app` no longer references `rhd_mcp_client`). Add a short note at the top of the MCP section:

```markdown
> **Note:** Scenario-level MCP integration was removed; MCP tools are now
> provided by the `rhd_plugin_mcp` plugin (see features/mcp-plugin.md).
```

Do not rewrite the rest of `scenarios.md` in this phase.

## Verification

- `grep -rn "mcp-tools" memory/` returns nothing.
- Links in `MEMORY.md` and `features/plugins.md` resolve to existing files.
- New doc contains no implementation internals beyond what's user-facing
  (per the product-view rule in `memory/MEMORY.md`).

## Implementation Notes

1. **Why a new file name:** the feature is now "MCP plugin", not "MCP tools"; the old name implied the scenario feature. Descriptive, context-rich naming per project conventions.
2. Keep `memory/features/mcp-tools.md` out of git history rewriting — plain `git rm` + new file.

## Dependencies

- Requires Phases 2–5 for accurate behavior documentation.
