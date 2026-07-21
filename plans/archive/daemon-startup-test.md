# Daemon Startup Test Plan

## Goal

Add a new test to `rhd_test` that verifies the daemon starts successfully within 5 seconds. This test should:
1. Start the daemon process
2. Wait up to 5 seconds for the "WebSocket listening" message
3. Pass if the message is received, fail otherwise
4. Be runnable via a dedicated command-line option

## Implementation Steps

### 1. Create new test module: `packages/rhd_test/src/daemon_startup_test.rs`

Create a new file with a function `run_daemon_startup_test()` that:
- Spawns the daemon with the same arguments as in `standard_test.rs`
- Reads stdout line by line
- Waits up to 5 seconds for "WebSocket listening" message
- Returns `(bool, String)` tuple (failed status, log output)
- Kills the daemon after the test completes

Key differences from `standard_test.rs`:
- Timeout is 5 seconds instead of 10
- No mock server needed (daemon will fail to connect to AI, but that's fine for startup test)
- No scenario execution
- No validation of requests/logs/meta.json
- Simpler: just check if daemon starts

### 2. Update `packages/rhd_test/src/args.rs`

Add a new variant to the `Commands` enum:
```rust
/// Run daemon startup test only
DaemonStartup,
```

### 3. Update `packages/rhd_test/src/main.rs`

- Add `mod daemon_startup_test;`
- Add handling for the new `Commands::DaemonStartup` variant in the match statement
- Call `daemon_startup_test::run_daemon_startup_test().await`
- Print PASS/FAIL and exit with appropriate code

## File Changes

| File | Change |
|------|--------|
| `packages/rhd_test/src/daemon_startup_test.rs` | New file with startup test logic |
| `packages/rhd_test/src/args.rs` | Add `DaemonStartup` command variant |
| `packages/rhd_test/src/main.rs` | Add module import and command handling |

## Usage

After implementation, the test can be run with:
```bash
cargo run -p rhd_test -- daemon-startup
```

Or via mise (if added):
```bash
mise run test-e2e -- daemon-startup
```

## Success Criteria

- Test passes when daemon prints "WebSocket listening" within 5 seconds
- Test fails when daemon does not print the message within 5 seconds
- Daemon is properly cleaned up (killed) after test completes
- Command-line option works correctly
