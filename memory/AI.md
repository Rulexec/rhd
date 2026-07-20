# RHD Project Knowledge Base

## Project Overview

RHD is a Rust-based automation tool for AI-assisted task execution. It uses a daemon/client architecture where a long-running daemon process executes scenarios (action chains) on behalf of client requests via Unix socket IPC. The system also includes a persistent chat feature for direct AI conversations with streaming responses and MCP tool integration.

## Multi-Crate Workspace Structure

```
rhd/
├── Cargo.toml (workspace root)
├── plans/            # Implementation plans
│   ├── archive/      # Completed/historical plans
│   └── milestones/   # Feature milestone plans
├── frontend/         # Svelte web UI
├── memory/           # This knowledge base (split by topic)
│   └── features/     # Product-scoped feature documentation
├── packages/
│   ├── rhd_util/     # Shared error types, utilities, env var substitution
│   ├── rhd_ai/       # OpenAI-compatible AI client
│   ├── rhd_api/      # Shared IPC types, protocol definitions, execution tracking types
│   ├── rhd_db/       # SQLite database for scenario ID persistence
│   ├── rhd_mcp_client/ # MCP protocol client for tool usage
│   ├── rhd_chat/     # Chat manager, tool loop, MCP integration for chat
│   ├── rhd_app/      # Main binary (daemon + client)
│   └── rhd_test/     # E2E test runner with mock AI server
```

## Documentation Structure

### Product-View vs Implementation

The knowledge base is organized into two layers:

**Top-level files** (`chat.md`, `configuration.md`, `scenarios.md`, etc.):
- **Implementation details**: crate APIs, database schemas, protocols, internal architecture
- **When to read**: When implementing or modifying code in specific areas
- **Contains**: Function signatures, struct definitions, database schemas, protocol messages, key file paths

**Features files** (`features/chat.md`, `features/configuration.md`, etc.):
- **Product-view only**: what the feature does, user interactions, behavior, configuration
- **When to read**: When understanding what a feature does from a user perspective
- **Contains**: User workflows, UI behavior, configuration options, error handling from user perspective
- **Does NOT contain**: Database schemas, internal crate APIs, protocol details, key file paths

**Pattern**: Features files describe "what it does" (product behavior), top-level files describe "how it's built" (implementation).

## Knowledge Base Index

**IMPORTANT**: Always read [development.md](development.md) before starting any work. It contains project conventions, testing practices, and commit guidelines.

Detailed documentation is split into topic-specific files. Read the relevant file when you need deep knowledge about a specific area:

| File | When to read |
|------|-------------|
| [architecture.md](architecture.md) | When working on crate structure, core component APIs, or understanding how crates relate to each other |
| [scenarios.md](scenarios.md) | When implementing or modifying scenario execution, action types, MCP tool integration, skip conditions, or placeholder resolution |
| [configuration.md](configuration.md) | When working on model configs, credentials, CLI arguments, `rhd.yaml`, env var substitution, or model aliases |
| [protocols.md](protocols.md) | When working on IPC (Unix socket), WebSocket protocol, CWD propagation, or client-daemon communication |
| [chat.md](chat.md) | When working on ChatManager, chat persistence, chat events, streaming, or chat-related WebSocket handlers |
| [logging.md](logging.md) | When working on execution logs, `meta.json` format, log output format, or step timing/tracking |
| [frontend.md](frontend.md) | When working on the Svelte web UI, chat stores, WebSocket client, or frontend components |
| [development.md](development.md) | When planning features, running tests, committing code, or needing to understand project conventions and error handling |
| [file-structure.md](file-structure.md) | When you need to find which file contains specific functionality or understand the project layout |
| [frontend-e2e.md](frontend-e2e.md) | When working on frontend E2E tests, test utilities, or test infrastructure |
| [backend-e2e.md](backend-e2e.md) | When working on backend E2E tests, rhd_test crate, mock server, or test scenarios |
| [debugging.md](debugging.md) | When any test fails |

## Token Saving Guidelines

When exploring the RHD codebase, **ALWAYS prioritize tokensave tools** over brute-force file reads or wide directory scans. The tokensave MCP server provides indexed access to code structure, call graphs, and dependencies, making it significantly more efficient than manual file exploration.

### Preferred Approach

Use these tokensave tools to understand code architecture:

- **`tokensave_context`** — Get relevant symbols, relationships, and code snippets for a task. Use this first to understand what code is relevant to your work.
- **`tokensave_search`** — Find symbols by name or keyword. Use this to locate specific functions, structs, traits, or modules.
- **`tokensave_related`** — Discover related symbols and dependencies. Use this to understand call chains, trait implementations, and code relationships.
- **`tokensave_body`** — Retrieve the full source of a symbol by name. Use this when you need to see the implementation details of a specific function or type.

### What to Avoid

**Do NOT** use these approaches for code exploration:

- ❌ Brute-force file reads (`read_file` on multiple files to "find" code)
- ❌ Wide directory scans (`list_files` with `recursive: true` to explore structure)
- ❌ Manual grep/search across files to locate functionality
- ❌ Reading entire files to understand their purpose

### Example: Finding Tool Loop Code

Instead of scanning directories or reading files to find where the tool loop is implemented, use tokensave:

```
tokensave_search(query="tool loop")
```

This immediately returns that the tool loop code is in [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs), along with related test functions and documentation references — no file scanning required.

### When to Use File Reads

File reads are appropriate **only after** you've identified the specific file and line range you need to examine via tokensave. Use `read_file` to:

- View the full implementation of a symbol you've already located
- Read configuration files, documentation, or test data
- Examine specific line ranges when you know exactly what you need

## Product Features

Product-scoped feature documentation (what the feature does, not how it's implemented):

| File | When to read |
|------|-------------|
| [features/scenario-execution.md](features/scenario-execution.md) | Understanding how scenarios run, action types, placeholders, abort, pause/resume |
| [features/chat.md](features/chat.md) | Understanding chat feature, streaming, model selection, message editing, delete all chats, auto-scroll |
| [features/mcp-tools.md](features/mcp-tools.md) | Understanding MCP tool integration, built-in tools, flags, skip conditions, tool call error UI |
| [features/configuration.md](features/configuration.md) | Understanding config files, credentials, model aliases, CLI arguments |
| [features/frontend-ui.md](features/frontend-ui.md) | Understanding web UI structure, tabs, routing, notifications, state export/import, markdown rendering, new chat dialog |
| [features/testing.md](features/testing.md) | Understanding E2E test infrastructure, mock server, test utilities |
| [features/logging-monitoring.md](features/logging-monitoring.md) | Understanding log files, meta.json, WebSocket events, notifications |
| [features/projects.md](features/projects.md) | Understanding projects feature, project structure, attaching projects to chats, MCP server lifecycle, system prompt injection |
