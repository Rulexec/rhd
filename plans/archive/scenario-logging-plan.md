# Log Loaded Scenario Names on Daemon Startup

## Goal
Add logging to display loaded scenario names when daemon starts.

## Current Flow
1. [`main.rs::run_daemon_command()`](packages/rhd_app/src/main.rs:38) calls [`scenario::load_scenarios_dir()`](packages/rhd_app/src/scenario/loader.rs:80)
2. Returns `HashMap<String, Scenario>` with loaded scenarios
3. Passes scenarios to [`daemon::run_daemon()`](packages/rhd_app/src/daemon.rs:19)
4. Daemon logs "listening on {socket_path}" at line 47

## Implementation Plan

### Step 1: Add scenario logging in daemon.rs
Location: [`packages/rhd_app/src/daemon.rs`](packages/rhd_app/src/daemon.rs:47), after state creation (line 42), before "listening on" log

Add logging statement:
```rust
let scenario_names: Vec<&String> = state.scenarios.keys().collect();
eprintln!(
    "loaded {} scenario{}: {}",
    scenario_names.len(),
    if scenario_names.len() == 1 { "" } else { "s" },
    scenario_names.into_iter().cloned().collect::<Vec<_>>().join(", ")
);
```

### Step 2: Handle empty scenario case
If no scenarios loaded, log:
```rust
if state.scenarios.is_empty() {
    eprintln!("no scenarios loaded");
}
```

## Logging Pattern
- Follow existing pattern: unconditional startup info (like "listening on")
- Always show scenario count and names on daemon startup
- Format: "loaded N scenario(s): name1, name2, ..."

## Files to Modify
- [`packages/rhd_app/src/daemon.rs`](packages/rhd_app/src/daemon.rs:47) - add scenario logging after line 42

## Testing
- Run daemon with test scenarios in `test_e2e/scenarios/`
- Verify log output shows correct scenario names
- Test with 0, 1, and multiple scenarios
