# Plan: Preserve stdout/stderr order in stdoutStderr placeholder (line-based)

## Goal

Change `stdoutStderr` placeholder behavior to preserve output order line-by-line, instead of concatenating stdout then stderr.

## Current Behavior

- [`execute_run_command()`](packages/rhd_app/src/scenario/executor.rs:79) uses `command.output().await` which captures stdout and stderr into separate buffers
- [`StepResult`](packages/rhd_app/src/scenario/placeholder.rs:4) stores `stdout: String` and `stderr: String` separately
- [`resolve_single()`](packages/rhd_app/src/scenario/placeholder.rs:58) handles `stdoutStderr` by concatenating: `format!("{}{}", result.stdout, result.stderr)`
- Result: all stdout appears first, then all stderr, regardless of actual output order

## Desired Behavior

- `stdoutStderr` should contain interleaved output line-by-line in the order lines were produced
- Individual `stdout` and `stderr` placeholders should continue working as before (separate content)
- Line-based ordering prevents crippy results from byte-level interleaving

## Implementation Approach

Add new field to `StepResult` to store combined output collected during execution:

1. **Modify `StepResult` struct** ([`placeholder.rs:4`](packages/rhd_app/src/scenario/placeholder.rs:4))
   - Add field: `pub stdout_stderr: String` (combined line-interleaved output)
   - Keep existing `stdout` and `stderr` fields for individual access

2. **Modify `execute_run_command()`** ([`executor.rs:79`](packages/rhd_app/src/scenario/executor.rs:79))
   - Instead of `command.output().await`, manually spawn child process with piped stdout/stderr
   - Use `tokio::io::BufReader` with `read_line()` to read line-by-line
   - Use `tokio::sync::mpsc` channel to collect lines with source tag (stdout/stderr)
   - Spawn async tasks for stdout and stderr that send lines to channel
   - Main task receives lines from channel and appends to combined buffer
   - Also collect into separate stdout/stderr strings for individual placeholders

3. **Update `resolve_single()`** ([`placeholder.rs:58`](packages/rhd_app/src/scenario/placeholder.rs:58))
   - Change `stdoutStderr` case to return `result.stdout_stderr.clone()` instead of concatenating

4. **Update `execute_ai_chat()`** ([`executor.rs:215`](packages/rhd_app/src/scenario/executor.rs:215))
   - Set `stdout_stderr: String::new()` in success case (AI chat doesn't produce stdout/stderr)

5. **Update tests** ([`placeholder.rs:72`](packages/rhd_app/src/scenario/placeholder.rs:72))
   - Update existing tests to include `stdout_stderr` field in `StepResult` construction
   - Add new test verifying `stdoutStderr` returns line-interleaved output

### Implementation details for line-based concurrent reading

```rust
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

// In execute_run_command:
let mut child = command.spawn()?;

let stdout = child.stdout.take().unwrap();
let stderr = child.stderr.take().unwrap();

let (tx, mut rx) = mpsc::channel::<(String, String)>(100); // (source, line)

let tx_out = tx.clone();
let stdout_task = tokio::spawn(async move {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let _ = tx_out.send(("stdout".to_string(), line.clone())).await;
            }
            Err(_) => break,
        }
    }
});

let tx_err = tx.clone();
let stderr_task = tokio::spawn(async move {
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let _ = tx_err.send(("stderr".to_string(), line.clone())).await;
            }
            Err(_) => break,
        }
    }
});

drop(tx); // Close sender so rx will end when tasks complete

let mut combined = String::new();
let mut stdout_buf = String::new();
let mut stderr_buf = String::new();

while let Some((source, line)) = rx.recv().await {
    combined.push_str(&line);
    match source.as_str() {
        "stdout" => stdout_buf.push_str(&line),
        "stderr" => stderr_buf.push_str(&line),
        _ => {}
    }
}

let _ = tokio::join!(stdout_task, stderr_task);
let status = child.wait().await?;
```

Channel preserves FIFO order, so lines appear in `combined` in the order they were read.

## Files to modify

1. [`packages/rhd_app/src/scenario/placeholder.rs`](packages/rhd_app/src/scenario/placeholder.rs)
   - Add `stdout_stderr: String` field to `StepResult`
   - Update `resolve_single()` to use new field for `stdoutStderr`
   - Update tests

2. [`packages/rhd_app/src/scenario/executor.rs`](packages/rhd_app/src/scenario/executor.rs)
   - Rewrite `execute_run_command()` to capture line-interleaved output
   - Update `execute_ai_chat()` to set `stdout_stderr` field

## Risks

- **Line buffering**: `read_line()` waits for newline. Commands that write partial lines may delay output. Acceptable for typical CLI tools.
- **Performance**: Channel overhead minimal for typical command output (KB-MB range).
- **Complexity**: Manual stream handling more complex than `command.output()`. Need careful error handling.

## Success criteria

- `stdoutStderr` placeholder returns output in line-interleaved order, not concatenated
- Individual `stdout` and `stderr` placeholders continue working
- Existing tests pass
- New test verifies line-based ordering behavior
- E2E tests pass
