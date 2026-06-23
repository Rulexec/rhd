# Log Format Changes Plan

## Goal

Move context prefixes inside `=====` markers in log output.

### Current vs Desired Format

| Context | Current | Desired |
|---------|---------|---------|
| Scenario start | `===== executing scenario =====\n<scenarioName>` | `===== <scenarioName>: executing scenario =====` |
| Command step | `<step>: ===== running command =====` | `===== <step>: running command =====` |
| AI step | `<step>: ===== AI request =====` | `===== <step>: AI request =====` |
| Output step | `===== output step =====\n<text>` | `===== <stepName>: output step =====\n<text>` |

## Files to Modify

### 1. [`packages/rhd_app/src/log.rs`](packages/rhd_app/src/log.rs)

#### Change `log()` method (line 33-41)

**Current:**
```rust
pub fn log(&mut self, header: &str, body: &str) {
    let block = format!("===== {header} =====\n{body}\n");
```

**New:**
```rust
pub fn log(&mut self, prefix: &str, header: &str, body: &str) {
    let block = if body.is_empty() {
        format!("===== {prefix}: {header} =====\n")
    } else {
        format!("===== {prefix}: {header} =====\n{body}\n")
    };
```

#### Change `log_step()` method (line 43-51)

**Current:**
```rust
let block = format!("{step}: ===== {header} =====\n{body}\n");
```

**New:**
```rust
let block = format!("===== {step}: {header} =====\n{body}\n");
```

### 2. [`packages/rhd_app/src/scenario/executor.rs`](packages/rhd_app/src/scenario/executor.rs)

| Line | Current | New |
|------|---------|-----|
| 51 | `sink.log("executing scenario", scenario_name)` | `sink.log(scenario_name, "executing scenario", "")` |
| 67-69 | `Action::Output(output) => { let resolved = ...; sink.log("output step", &resolved); }` | Extract `step_name` from `output.name` (like RunCommand/AiChat do), then `sink.log(&step_name, "output step", &resolved)` |

All `log_step()` calls unchanged — format change handled inside `log.rs`.

### 3. [`packages/rhd_app/src/daemon.rs`](packages/rhd_app/src/daemon.rs)

| Line | Current | New |
|------|---------|-----|
| 141 | `sink.log("unknown scenario", &name)` | `sink.log(&name, "unknown scenario", "")` |
| 160 | `sink.log("scenario execution error", &err.to_string())` | `sink.log("error", "scenario execution error", &err.to_string())` |

### 4. [`packages/rhd_test/src/main.rs`](packages/rhd_test/src/main.rs)

Update expected substrings (lines 413-426):

```rust
// Current                              // New
"===== executing scenario =====\nrhd_test"  →  "===== rhd_test: executing scenario ====="
"cmd1: ===== running command ====="         →  "===== cmd1: running command ====="
"cmd1: ===== command exit code ====="       →  "===== cmd1: command exit code ====="
"ai1: ===== AI request ====="              →  "===== ai1: AI request ====="
"ai1: ===== AI response ====="             →  "===== ai1: AI response ====="
"===== output step ====="                  →  "===== out1: output step ====="
```

### 5. [`AI.md`](AI.md)

Update log format section (lines 140-167) to reflect new format:

```
===== <scenarioName>: executing scenario =====

===== <stepName>: running command =====
<command> <args>

===== <stepName>: command output =====
[STDOUT] stdout line
[STDERR] stderr line

===== <stepName>: command exit code =====
<code>

===== <stepName>: AI request =====
model: <model>
----- system prompt -----
<prompt>
----- message -----
<message>

===== <stepName>: AI response =====
<response>

===== <stepName>: output step =====
<resolved output>
```

### 6. [`README.md`](README.md)

Remove "Log format" section (lines 83-110) — not important for user-facing docs.

## Implementation Steps

1. Modify `log.rs` — update `log()` signature/format and `log_step()` format
2. Update `executor.rs` — fix `log()` calls, extract step_name for Output action
3. Update `daemon.rs` — fix `log()` calls
4. Update `rhd_test/src/main.rs` — fix expected substrings
5. Update `AI.md` — update log format documentation
6. Remove log format section from `README.md`
7. Run `cargo build` and `cargo run -p rhd_test` to verify
