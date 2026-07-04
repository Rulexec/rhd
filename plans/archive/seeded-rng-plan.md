# Seeded RNG for E2E Tests

## Goal

Make `rhd_test` deterministic: accept `--seed` argument, use seeded RNG for all random generation, remove randomness from generated bash scripts, add `--repetitions` (default 10) to run tests in loop.

## Current Random Usage

[`packages/rhd_test/src/main.rs`](packages/rhd_test/src/main.rs):

1. **Line 134-146** — `generate_random_string()` uses `rand::thread_rng()` for AI response content
2. **Line 100-120** — `create_temp_script()` writes bash script using `/dev/urandom` and `$RANDOM`

## Changes

### 1. Add CLI Arguments

Add `clap` dependency to [`packages/rhd_test/Cargo.toml`](packages/rhd_test/Cargo.toml).

Define struct:
```rust
#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 10)]
    repetitions: u32,
}
```

### 2. Replace `thread_rng()` with Seeded RNG

Use `rand::rngs::StdRng::seed_from_u64(seed)`.

Change `generate_random_string` signature:
```rust
fn generate_random_string(rng: &mut impl Rng, length: usize) -> String
```

### 3. Deterministic Bash Script

Remove `/dev/urandom` and `$RANDOM` from script. Pre-generate on Rust side:

```rust
fn create_temp_script(dir: &Path, rng: &mut impl Rng) {
    let output = generate_random_string(rng, 8);
    let exit_code = rng.gen_range(0..5);
    let script_content = format!(r#"#!/bin/sh
echo "{}"
exit {}
"#, output, exit_code);
    // write + chmod 755
}
```

Script now deterministic — same seed → same output + exit code.

### 4. Test Loop

Wrap main test logic in loop:

```rust
for i in 0..args.repetitions {
    let iter_seed = args.seed + i as u64;
    println!("\n=== Repetition {}/{} (seed: {}) ===", i + 1, args.repetitions, iter_seed);
    // create new StdRng::seed_from_u64(iter_seed) for this iteration
    // run full test cycle
}
```

**Decision**: Create new RNG per repetition with `seed + i`. Print iteration seed so failed test can be reproduced with `--seed <iter_seed> --repetitions 1`.

### 5. Per-Repetition State

Each iteration needs:
- Reuse mock AI server (port stays same across all iterations)
- Reset request log between iterations (clear `requests.lock().unwrap().clear()`)
- New temp dir with new script (generated with iteration RNG)
- Fresh daemon spawn
- Separate validation

Refactor current `main()` body into `run_single_test(seed: u64, port: u16, requests: SharedRequests, response: SharedResponse) -> bool` function.

Mock server started once in `main()` before loop, port passed to each iteration.

## File Changes

| File | Change |
|------|--------|
| [`packages/rhd_test/Cargo.toml`](packages/rhd_test/Cargo.toml) | Add `clap = { version = "4", features = ["derive"] }` |
| [`packages/rhd_test/src/main.rs`](packages/rhd_test/src/main.rs) | Add CLI parsing, seeded RNG, deterministic script, test loop |

## Implementation Steps

1. Add `clap` dependency to `Cargo.toml`
2. Define `Args` struct with `seed` and `repetitions`
3. Parse args in `main()`
4. Start mock server once in `main()`, get port + shared state
5. Extract test logic into `run_single_test(seed: u64, port: u16, requests: SharedRequests, response: SharedResponse) -> bool`
6. Modify `generate_random_string` to accept `&mut impl Rng`
7. Modify `create_temp_script` to accept `&mut impl Rng`, pre-generate output + exit code
8. Add loop in `main()` calling `run_single_test(seed + i, port, ...)` for each repetition
9. Aggregate results: track failures across all repetitions
10. Update [`AI.md`](AI.md) with new CLI usage

## Success Criteria

- `cargo run -p rhd_test -- --seed 123 --repetitions 5` runs 5 deterministic iterations
- Same seed → same test behavior (output, exit codes, AI responses)
- Generated bash scripts contain no `/dev/urandom` or `$RANDOM`
- Default: 10 repetitions, seed 42

## Risks

- Socket file cleanup between iterations → already handled by `_ = std::fs::remove_file`
- Daemon from previous iteration not fully killed → ensure `daemon.kill().await` + wait before next iteration
