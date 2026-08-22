# Phase 1: Remove Legacy Packages

## Overview

This phase removes all legacy packages that are no longer needed in the new architecture. This includes the old daemon/client binary, FSM framework, test runner, chat manager, old AI client, and old API types.

**Scope:**
- Remove 6 packages from workspace
- Delete package directories
- Verify compilation succeeds

**Out of scope:**
- Code cleanup in remaining packages (will be done later)
- Memory file updates (Phase 4)

## Files to Modify

### 1. `Cargo.toml` (workspace root)

**Modifications:**
Remove the following lines from the `members` array:

```toml
[workspace]
members = [
    "packages/rhd_util",
    "packages/rhd_ai_client",        # Keep
    "packages/rhd_mock_ai_provider", # Keep
    "packages/rhd_mcp_client",       # Keep
    "packages/rhd_db",               # Keep
    "packages/rhd_chat_api",         # Keep
    "packages/rhd_chat_server",      # Keep
    "packages/rhd_chat_client",      # Keep
    "plugins/rhd_plugin_ai_completions", # Keep
    # REMOVED:
    # "packages/rhd_ai",             # Remove - superseded by rhd_ai_client
    # "packages/rhd_api",            # Remove - not needed anymore
    # "packages/rhd_chat",           # Remove - depends on rhd_fsm
    # "packages/rhd_app",            # Remove - old daemon/client
    # "packages/rhd_fsm",            # Remove - only used by rhd_chat
    # "packages/rhd_test",           # Remove - depends on rhd_app
]
```

**Result:**
```toml
[workspace]
members = [
    "packages/rhd_util",
    "packages/rhd_ai_client",
    "packages/rhd_mock_ai_provider",
    "packages/rhd_mcp_client",
    "packages/rhd_db",
    "packages/rhd_chat_api",
    "packages/rhd_chat_server",
    "packages/rhd_chat_client",
    "plugins/rhd_plugin_ai_completions",
]
resolver = "2"
```

## Directories to Delete

Delete the following directories completely:

1. `packages/rhd_app/` - Old daemon/client binary
2. `packages/rhd_fsm/` - Finite state machine framework
3. `packages/rhd_test/` - E2E test runner
4. `packages/rhd_chat/` - Chat manager and tool loop
5. `packages/rhd_ai/` - Old OpenAI-compatible AI client
6. `packages/rhd_api/` - Old shared types

**Command:**
```bash
rm -rf packages/rhd_app packages/rhd_fsm packages/rhd_test packages/rhd_chat packages/rhd_ai packages/rhd_api
```

## Verification

After removal, verify the workspace compiles successfully:

```bash
cargo check
```

**Expected result:**
- No compilation errors
- All remaining packages compile successfully
- No references to removed packages

## Implementation Notes

1. **Order of removal:** Remove all packages at once to avoid intermediate compilation failures
2. **No code cleanup:** Do not clean up unused code in remaining packages - this will be done in a later phase
3. **Dependencies:** All removed packages are only used by other removed packages, so no remaining code should break

## Dependencies

- This is the first phase - no dependencies on other phases
- Must be completed before Phase 2, 3, and 4

## Success Criteria

- [ ] All 6 package directories deleted
- [ ] `Cargo.toml` updated with correct members list
- [ ] `cargo check` passes without errors
- [ ] No references to removed packages in remaining code
