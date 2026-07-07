# Fix Checks Skill

## Overview

This skill documents how to run static analysis checks, identify warnings/errors, and fix them systematically.

## Running Checks

### All Checks

Run both cargo and svelte checks sequentially:

```bash
mise run check
```

### Individual Checks

**Cargo (Rust) checks:**

```bash
mise run check-cargo
```

Runs `cargo check` — validates Rust compilation without producing binaries.

**Svelte checks:**

```bash
mise run check-svelte
```

Runs `svelte-check` in the frontend directory — validates TypeScript types and Svelte-specific linting.

## Filtering Output

### Cargo Warnings

To see only warnings and errors:

```bash
mise run check-cargo 2>&1 | grep -E "warning:|error:"
```

To see warnings with file locations:

```bash
mise run check-cargo 2>&1 | grep -E "warning:|-->"
```

To count total warnings:

```bash
mise run check-cargo 2>&1 | grep "warning:" | wc -l
```

### Svelte Warnings

Svelte-check output is already concise. To verify zero warnings:

```bash
mise run check-svelte 2>&1 | grep "found.*errors and.*warnings"
```

Expected output: `svelte-check found 0 errors and 0 warnings`

## Fixing Cargo Warnings

### Unused Imports

**Warning pattern:** `warning: unused import: \`Name\``

**Fix:** Remove the unused import from the `use` statement.

Example:
```rust
// Before
use crate::execution::{ExecutionHandle, ResumeAction};

// After (if ResumeAction unused)
use crate::execution::ExecutionHandle;
```

### Unused Variables

**Warning pattern:** `warning: unused variable: \`name\``

**Fix options:**
1. Prefix with underscore: `_name`
2. Remove the variable if truly unnecessary
3. Use the variable if it was intended to be used

Example:
```rust
// Before
pub async fn send_message(
    mcp_cache: &McpServerCache,  // unused
) -> Result<i64, ChatError> {

// After
pub async fn send_message(
    _mcp_cache: &McpServerCache,  // prefixed with _
) -> Result<i64, ChatError> {
```

### Unused Function Parameters

Same as unused variables — prefix with underscore or remove if not needed for API compatibility.

### Dead Code (Unused Functions/Methods)

**Warning pattern:** `warning: function \`name\` is never used`

**Fix options:**
1. Remove the function if truly unnecessary
2. Add `#[allow(dead_code)]` if intentionally kept for future use
3. Make the function public if it should be part of the API

Example:
```rust
// Option 1: Remove
// (delete the function entirely)

// Option 2: Suppress warning
#[allow(dead_code)]
pub async fn active_stream_count(&self) -> usize {
    self.active_streams.lock().await.len()
}
```

### Dead Code (Unused Struct Fields)

**Warning pattern:** `warning: field \`name\` is never read`

**Fix:** Add `#[allow(dead_code)]` attribute to the field.

Example:
```rust
pub struct DaemonState {
    #[allow(dead_code)]
    pub chat_db: Arc<ChatDb>,
    // other fields...
}
```

### Unused Re-exports

**Warning pattern:** `warning: unused import: \`module::Name\`` for `pub use` statements

**Fix:** Remove the re-export if not used by external crates.

Example:
```rust
// Before
pub use error::ExecuteError;
pub use error::ExecuteOutput;  // unused

// After
pub use error::ExecuteError;
```

### Assigned but Never Read

**Warning pattern:** `warning: variable \`name\` is assigned to, but never used`

**Fix:** Remove the variable assignment or use the variable.

Example:
```rust
// Before
let mut current_model_name = model_name;
// ... later ...
current_model_name = new_model_name;  // assigned but never read

// After
// Remove the variable entirely if not needed
```

## Fixing Svelte Warnings

### A11y: Static Element Interactions

**Warning pattern:** `<div>` with a click or keydown handler must have an ARIA role

**Fix:** Add appropriate `role` and `tabindex` attributes.

Example:
```svelte
<!-- Before -->
<div class="overlay" on:click on:keydown={handleKeydown}>

<!-- After -->
<div class="overlay" role="button" tabindex="0" on:click on:keydown={handleKeydown}>
```

Common roles:
- `role="button"` — for clickable elements
- `role="dialog"` — for modal dialogs (also needs `tabindex="-1"`)
- `role="region"` — for landmark sections

### A11y: Interactive Role Focus

**Warning pattern:** Elements with interactive role must have tabindex

**Fix:** Add `tabindex` attribute.

```svelte
<!-- Before -->
<div role="dialog">

<!-- After -->
<div role="dialog" tabindex="-1">
```

Use `tabindex="-1"` for elements that should be focusable programmatically but not via tab navigation.
Use `tabindex="0"` for elements that should be in the natural tab order.

### Deprecated Event Syntax

**Warning pattern:** Using `on:click` is deprecated. Use `onclick` instead

**Fix:** Replace `on:event` with `onevent` attribute.

Example:
```svelte
<!-- Before -->
<button on:click={() => doSomething()}>

<!-- After -->
<button onclick={() => doSomething()}>
```

Applies to all event handlers: `onclick`, `onkeydown`, `onsubmit`, etc.

## Verification Workflow

1. **Run checks:**
   ```bash
   mise run check
   ```

2. **If warnings found, filter to see them:**
   ```bash
   mise run check-cargo 2>&1 | grep -E "warning:|-->"
   ```

3. **Fix warnings systematically** — one file at a time or by warning type

4. **Re-run checks to verify:**
   ```bash
   mise run check
   ```

5. **Run tests to ensure no regressions:**
   ```bash
   mise run test-all
   ```

## Common Patterns

### Multiple Warnings in One File

When a file has multiple warnings, batch all fixes for that file into a single edit operation to minimize round-trips.

### Svelte: A11y Warnings on Nested Elements

When fixing nested interactive elements, ensure each has appropriate role and tabindex:

```svelte
<div role="button" tabindex="0" on:click>
  <div role="dialog" tabindex="-1" on:click|stopPropagation>
    <!-- content -->
  </div>
</div>
```

### Intentionally Dead Code

For code that's intentionally kept (e.g., for future features, API compatibility), always add `#[allow(dead_code)]` with a comment explaining why:

```rust
#[allow(dead_code)]  // Kept for future reload functionality
pub config_file: PathBuf,
```

## Testing After Fixes

Always run the full test suite after fixing warnings to ensure no regressions:

```bash
mise run test-all
```

This includes:
- Frontend unit tests
- Frontend e2e tests
- Cargo unit tests
- Backend e2e tests
