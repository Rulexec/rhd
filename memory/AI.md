# RHD Project Knowledge Base

## Project Overview

RHD is a Rust-based automation tool for AI-assisted task execution. It uses a daemon/client architecture where a long-running daemon process executes scenarios (action chains) on behalf of client requests via Unix socket IPC. The system also includes a persistent chat feature for direct AI conversations with streaming responses.

## Multi-Crate Workspace Structure

```
rhd/
├── Cargo.toml (workspace root)
├── plans/            # Implementation plans
├── frontend/         # Svelte web UI
├── memory/           # This knowledge base (split by topic)
├── packages/
│   ├── rhd_util/     # Shared error types, utilities, env var substitution
│   ├── rhd_ai/       # OpenAI-compatible AI client
│   ├── rhd_api/      # Shared IPC types, protocol definitions, execution tracking types
│   ├── rhd_db/       # SQLite database for scenario ID persistence
│   ├── rhd_mcp_client/ # MCP protocol client for tool usage
│   ├── rhd_app/      # Main binary (daemon + client)
│   └── rhd_test/     # E2E test runner with mock AI server
```

## Knowledge Base Index

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
