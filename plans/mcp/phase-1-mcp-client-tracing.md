# Phase 1: Migrate `rhd_mcp_client` from prints to `tracing`

## Overview

Replace the `eprintln!`-based debug output in `rhd_mcp_client` with structured `tracing` logs, and remove the now-unnecessary `DEBUG` static. This makes the crate a good citizen for the tracing-based plugins (notably the new `rhd_plugin_mcp`, Phase 2+) and lets wire-level MCP traffic be controlled via `RUST_LOG` instead of a global flag.

**Scope:**
- In: `packages/rhd_mcp_client` only — `transport.rs` debug prints, `lib.rs` `DEBUG` static, `Cargo.toml` dependency.
- Out: any behavior changes to the protocol/transport logic; changes to other crates.

**Depends on:** nothing. Can run in parallel with Phase 2.

## Files to Modify

### 1. `packages/rhd_mcp_client/Cargo.toml`

**Modification:** add `tracing` (workspace dep) to `[dependencies]`:

```toml
[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

### 2. `packages/rhd_mcp_client/src/transport.rs`

**Modification A — request logging (currently lines 61–64):**

Before:
```rust
let request_json = serde_json::to_string(request)?;
if crate::DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
    eprintln!("[MCP] writing to stdin: {}", request_json);
}
```

After:
```rust
let request_json = serde_json::to_string(request)?;
tracing::debug!(request = %request_json, "MCP request → stdin");
```

**Modification B — response logging (currently lines 82–84):**

Before:
```rust
if crate::DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
    eprintln!("[MCP] read from stdout: {}", response_line.trim());
}
```

After:
```rust
tracing::debug!(response = %response_line.trim(), "MCP response ← stdout");
```

No other changes to `transport.rs`. Note: `command.stderr(std::process::Stdio::inherit())` at spawn time stays as-is — that is the MCP server's own log stream, not crate prints.

### 3. `packages/rhd_mcp_client/src/lib.rs`

**Modification:** remove the `DEBUG` static and its unused import.

Before (lines 6–10):
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

pub static DEBUG: AtomicBool = AtomicBool::new(false);
```

After:
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
```

`grep -rn "DEBUG" packages/rhd_mcp_client/src` must return nothing afterwards. (Verified earlier: the only references are `transport.rs` lines 62/82 and the definition itself.)

## Tests

No new tests in this phase (logging-only change). Verify no regressions:

```bash
mise run check-cargo   # workspace compiles
mise run test-cargo    # existing rhd_mcp_client tests still pass
```

## Implementation Notes

1. **Why `tracing::debug!` unconditionally:** the subscriber's `EnvFilter` already gates emission; a second in-crate boolean flag is redundant and invisible to log configuration.
2. **Structured fields** (`request = %...`, `response = %...`) allow log consumers to filter wire traffic per direction.
3. **Consumers:** no other crate references `rhd_mcp_client::DEBUG` (verified by search), so removal is safe.
4. **Known limitation (do not fix here):** `StdioTransport::send_request` reads exactly one line per request. An unsolicited server notification would desync the line protocol. Out of scope; note it for the plugin's README (Phase 2) as a caveat when choosing MCP servers.

## Dependencies

- None.
- Must be completed before Phase 3 only in the sense that Phase 3's pool benefits from crate-level tracing; there is no compile-time coupling beyond the new `tracing` dependency.
