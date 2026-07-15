# Fix Checks Skill

## Overview

This skill documents how to run static analysis checks, identify warnings/errors, and fix them systematically.

## IMPORTANT: Clean State Requirement

**This skill MUST NOT be used during implementation of sub-plans.**

The fix-checks skill should only be run when the codebase is in a clean, stable state. Dead code analysis assumes that unused code is truly unnecessary. During active development, functions may appear unused but are intended for upcoming features.

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

**Investigation process:**

1. **Search for existing `#[allow(dead_code)]` attributes:**
   ```bash
   grep -r "#\[allow(dead_code)\]" packages/ --include="*.rs" -B 2 -A 5
   ```
   Review each existing suppression to determine if the code is still needed.

2. **Check if the function is called anywhere:**
   - Search for the function name across the codebase
   - Check if it's used in tests (may be in `#[cfg(test)]` blocks)
   - Verify it's not part of a public API that external crates might use

3. **Determine if the code is truly dead:**
   - If the function is never called and not part of a planned feature → **remove it**
   - If the function is part of an incomplete feature being actively developed → this violates the clean state requirement; do not run fix-checks
   - If the function is a public API method → make it public or remove it

4. **Remove dead code:**
   - Delete the function entirely
   - Remove any associated tests
   - Update documentation if the function was documented

**Example:**
```rust
// Before: Function is never used
pub fn render_template(&self, name: &str) -> Option<String> {
    // implementation
}

// After: Remove the dead code
// (delete the function entirely)
```

**Avoid adding `#[allow(dead_code)]`** unless there's a compelling reason (e.g., trait implementation requirement, FFI boundary).

### Dead Code (Unused Struct Fields)

**Warning pattern:** `warning: field \`name\` is never read`

**Investigation process:**

1. **Check existing `#[allow(dead_code)]` on struct fields:**
   Review all fields with this attribute. Ask:
   - Is this field populated but never read? → Likely dead code
   - Is this field required for serialization/deserialization? → Keep it
   - Is this field part of a public API struct? → May be needed for API compatibility

2. **Determine if the field is truly dead:**
   - If the field is never read and not required for serialization → **remove it**
   - If the field is required for serde but never read in code → keep with `#[allow(dead_code)]` and add a comment explaining why
   - If the field is part of a public API → document why it's kept

3. **Remove dead fields:**
   - Remove the field from the struct definition
   - Remove any code that populates the field
   - Update tests that reference the field

**Example:**
```rust
// Before: Field is never read
pub struct DaemonState {
    #[allow(dead_code)]
    pub chat_db: Arc<ChatDb>,  // Never used
}

// After: Remove the dead field
pub struct DaemonState {
    // chat_db removed - was never used
}
```

**Keep `#[allow(dead_code)]` only when:**
- Required for serde serialization/deserialization
- Required by a trait implementation
- Part of FFI boundaries
- Documented with a clear reason for keeping

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
