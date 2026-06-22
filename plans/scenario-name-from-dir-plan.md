# Plan: Derive scenario name from directory name

## Problem

[`load_scenarios_dir()`](packages/rhd_app/src/scenario/loader.rs:80) uses `scenario.name` (from YAML `name:` field) as HashMap key at [line 100](packages/rhd_app/src/scenario/loader.rs:100). Two scenarios with same `name:` value → second overwrites first → only 1 loaded.

## Goal

Remove `name` field requirement from `scenario.yaml`. Scenario identifier = directory name (consistent with how models work: filename = model name).

## Changes

### 1. `packages/rhd_app/src/scenario/mod.rs`
- Remove `pub name: String` from [`Scenario`](packages/rhd_app/src/scenario/mod.rs:62) struct

### 2. `packages/rhd_app/src/scenario/loader.rs`
- [`load_scenarios_dir()`](packages/rhd_app/src/scenario/loader.rs:80): extract directory name from `path.file_name()`, use as HashMap key
- [`validate_scenario()`](packages/rhd_app/src/scenario/loader.rs:44): remove `scenario.name.is_empty()` check (lines 45-50)
- Insert into HashMap using directory name, not `scenario.name`

### 3. `packages/rhd_app/src/scenario/executor.rs`
- [`execute_scenario()`](packages/rhd_app/src/scenario/executor.rs:39): add `scenario_name: &str` parameter
- Replace `let scenario_name = &scenario.name;` (line 48) with using the parameter

### 4. `packages/rhd_app/src/daemon.rs`
- [`handle_request()`](packages/rhd_app/src/daemon.rs:127): pass `&name` (the HashMap key from IPC request) to `execute_scenario`

### 5. `packages/rhd_app/src/main.rs`
- No changes needed (client sends scenario name string, daemon looks up by key)

### 6. Test scenario YAMLs — remove `name:` field
- `test_e2e/scenarios/rhd_test/scenario.yaml`: remove line 1 (`name: rhd_test`)
- Any other test scenario YAMLs with `name:` field

### 7. Documentation updates
- **`AI.md`**: 
  - Remove `name: scenario_name` from scenario YAML example (line 67)
  - Update convention #6 (line 156): "Directory name is scenario identifier" (remove mention of `scenario.yaml` containing name)
- **`README.md`**:
  - Remove `name` from top-level fields table (line 92) — mark as not required / remove row
  - Remove `name: example` from example scenario YAML (line 155)

## Verification

- `cargo build` compiles
- `cargo test` passes
- `rhd_test` E2E passes
- Manual: two dirs with same or no `name:` field → both loaded
