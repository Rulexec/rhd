# README.md Update Plan

## Goal
Update README.md to reflect current implementation, focusing on YAML specifications and CLI arguments without implementation details.

## Current State Analysis
README.md is missing several features that exist in the codebase:
- CLI: `--ws-port`, `--db-dir` (daemon), `--modelAlias` (run)
- Config: `mcpDir`, `wsPort`, `dbDir`, `credentialsConfig`
- Model config: token pricing fields, alias support
- Credentials configuration section
- WebSocket server information

## Changes Required

### 1. Update Daemon CLI Table
Add missing flags to the daemon command table:
- `--ws-port PORT` — WebSocket server port (optional)
- `--db-dir DIR` — Directory for SQLite database (default: `rhd_db`)

### 2. Update Run CLI Section
Add `--modelAlias` flag documentation:
```bash
rhd run <scenario_name> [--socket PATH] [--modelAlias ALIAS=TARGET]
```

### 3. Update Configuration File Section
Add missing fields to `rhd.yaml` format:
- `mcpDir: mcp` — Directory containing MCP server configurations
- `wsPort: null` — WebSocket server port
- `dbDir: rhd_db` — Directory for SQLite database
- `credentialsConfig: null` — Path to credentials file

### 4. Update Models Section
Add optional token pricing fields:
- `inputTokenPrice` — Price per 1M input tokens
- `outputTokenPrice` — Price per 1M output tokens
- `priceTiers` — Tiered pricing based on token count

### 5. Add Model Alias Section
Document model alias feature:
```yaml
# models/medium.yaml
alias: gpt4
```
Explain that filename becomes alias name, resolves to target model.

### 6. Add CLI Model Alias Override Section
Document `--modelAlias` flag:
```bash
rhd run example --modelAlias medium=gpt4 --modelAlias small=qwen3
```

### 7. Add Credentials Configuration Section
Document credentials feature for separating API keys from config:
- `credentials.yaml` format
- `apiKey: { cred: keyName }` reference in model config
- Path resolution relative to `rhd.yaml`

### 8. Add WebSocket Server Section
Brief mention of WebSocket server for Web UI integration:
- Enabled via `--ws-port` or `wsPort` config
- Default disabled
- Used by frontend for real-time scenario monitoring

## Structure
Keep existing README structure, add new sections where appropriate:
1. Architecture (unchanged)
2. Install (unchanged)
3. CLI (update tables)
4. Configuration File (update fields)
5. Models (add pricing, alias)
6. Credentials (new section)
7. Scenarios (unchanged)
8. WebSocket (new brief section)
9. Execution Logs (unchanged)
10. Example scenario (unchanged)
11. Behavior notes (unchanged)

## Success Criteria
- README accurately reflects all CLI arguments from `cli.rs`
- README accurately reflects all config fields from `config.rs`
- Model config documentation includes all fields from `rhd_ai/src/config.rs`
- New features (alias, credentials, websocket) are documented
- No implementation details (Rust code, internal structures)
- Basic scenario example preserved
