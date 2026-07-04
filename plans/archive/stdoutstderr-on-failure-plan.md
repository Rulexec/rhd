# Fix: stdoutStderr placeholder empty on command failure

## Problem

When a `runCommand` step fails (non-zero exit code), the `%stepName.stdoutStderr%` placeholder resolves to an empty string, even though stderr was captured and logged in verbose mode.

This prevents subsequent `aiChat` steps from seeing command failure output, making it impossible for AI to diagnose what went wrong.

## Root Cause

In [`packages/rhd_app/src/scenario/placeholder.rs:59`](packages/rhd_app/src/scenario/placeholder.rs:59):

```rust
"stdoutStderr" => {
    if result.success {
        result.stdout_stderr.clone()
    } else {
        String::new()
    }
}
```

The `stdoutStderr` field intentionally returns empty string when `success` is false.

## Analysis of Other Placeholders

Checked all command output placeholders in [`placeholder.rs:55-70`](packages/rhd_app/src/scenario/placeholder.rs:55):

- `stdout` (line 57): no success check ✓
- `stderr` (line 58): no success check ✓
- `stdoutStderr` (line 59-65): **has success check** ✗ (the bug)
- `exitCode` (line 56): no success check ✓
- `success` (line 66): returns success flag itself ✓
- `message` (line 67): no success check ✓
- `cwd` (line 68): no success check ✓

Only `stdoutStderr` has the problematic success check. `stdout` and `stderr` placeholders work correctly regardless of exit status.

## Solution

Remove the success check. Always return the captured `stdout_stderr` content regardless of exit status.

### Changes

1. **[`packages/rhd_app/src/scenario/placeholder.rs`](packages/rhd_app/src/scenario/placeholder.rs)**
   - Line 59-65: Simplify `stdoutStderr` match arm to always return `result.stdout_stderr.clone()`

2. **[`packages/rhd_app/src/scenario/placeholder.rs`](packages/rhd_app/src/scenario/placeholder.rs)** (tests)
   - Update test `stdout_stderr_empty_on_failure` (line 102) → rename to `stdout_stderr_available_on_failure`
   - Change assertion to expect `"out\nerr\n"` instead of `""`

## Impact

- `aiChat` steps can now access command output even when commands fail
- AI can diagnose failures using actual error messages
- No breaking change to other placeholder fields (`exitCode`, `success`, `stdout`, `stderr` remain unchanged)

## Verification

1. Run `cargo test` - all tests pass
2. Create test scenario with failing command followed by aiChat using `%cmd.stdoutStderr%`
3. Verify verbose log shows message contains captured output
4. Run E2E tests: `cargo run -p rhd_test`
