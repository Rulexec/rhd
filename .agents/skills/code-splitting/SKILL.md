# Code Splitting Skill

## Overview

This skill documents how to identify and split large source files (>400 lines) into smaller, logical modules. The goal is to maintain code readability and maintainability by ensuring **no single file exceeds 400 lines**, including test files.

## Checking File Sizes

### Backend (Rust) Files

To find the top 20 largest Rust source files (including tests):

```bash
mise run top-files-backend
```

This shows files sorted by line count (descending), with the total at the top.

### Frontend (TypeScript/Svelte) Files

To find the top 20 largest frontend source files (including tests):

```bash
mise run top-files-frontend
```

### Check for Files Exceeding 400 Lines

To see only files that exceed the 400-line limit:

```bash
# Backend files > 400 lines
mise run check-large-files-backend

# Frontend files > 400 lines
mise run check-large-files-frontend

# Both backend and frontend
mise run check-large-files
```

## Splitting Strategy

### Target Size

**Maximum file size: 400 lines**

Any file exceeding 400 lines should be split into logical modules. This applies to:
- Source files
- Test files
- All other code files

### Splitting Order

1. **Start with the largest files** - Work through files in descending order of size
2. **Extract test modules first** - If a file has inline `#[cfg(test)]` modules, extract them to separate `tests.rs` or `tests/` directory
3. **Split by logical responsibility** - Group related functions/types into separate files
4. **Maintain public API** - Ensure the module's public interface remains unchanged

### Common Splitting Patterns

#### Pattern 1: Single File → Directory with mod.rs

Convert `file.rs` into `file/` directory:

```
file.rs → file/
         ├── mod.rs       # Module declarations and re-exports
         ├── types.rs     # Type definitions
         ├── handlers.rs  # Handler functions
         └── utils.rs     # Utility functions
```

**Example:** `packages/rhd_app/src/execution.rs` → `packages/rhd_app/src/execution/`

#### Pattern 2: Extract Test Module

Move inline tests to separate file:

```rust
// Before: file.rs
pub fn some_function() { }

#[cfg(test)]
mod tests {
    // test code
}

// After: file.rs
pub fn some_function() { }

#[cfg(test)]
mod tests;

// After: file/tests.rs
// test code
```

#### Pattern 3: Split by Functionality

Split large files by logical domains:

```
lib.rs (593 lines) → 
  ├── lib.rs          # Module declarations and re-exports
  ├── execution.rs    # Execution tracking types
  ├── chat.rs         # Chat-related DTOs and events
  └── ws.rs           # WebSocket protocol types
```

#### Pattern 4: Split Large Test Files

Split test files by test category:

```
tests.rs (938 lines) → tests/
                       ├── mod.rs              # Module declarations
                       ├── tool_tests.rs       # Tool-related tests
                       ├── integration_tests.rs # Integration tests
                       └── edge_cases.rs       # Edge case tests
```

### Implementation Steps

1. **Read the file** - Understand its structure and identify logical groupings
2. **Plan the split** - Decide which functions/types go into which files
3. **Create new files** - Write the split files with proper imports
4. **Update mod.rs** - Add module declarations and re-exports
5. **Delete old file** - Remove the original file (if converting to directory)
6. **Verify compilation** - Run `cargo check` to ensure no errors
7. **Run tests** - Execute `cargo test --workspace` to verify functionality

### Import Management

When splitting files, carefully manage imports:

- **Use absolute paths** for cross-module imports: `use crate::module::Type;`
- **Use `super::`** for parent module imports: `use super::ParentType;`
- **Re-export public types** in `mod.rs`: `pub use submodule::PublicType;`

Example:
```rust
// execution/mod.rs
mod handle;
mod tracker;

pub use handle::{ExecutionHandle, FinishedExecution};
pub use tracker::{ExecutionTracker, ResumeAction};
```

## Verification

### After Each Split

1. **Check compilation:**
   ```bash
   cargo check
   ```

2. **Run tests:**
   ```bash
   cargo test --workspace
   ```

3. **Verify no warnings:**
   ```bash
   cargo check 2>&1 | grep -E "warning:|error:"
   ```

### Final Verification

After completing all splits, verify no files exceed 400 lines:

```bash
# Check both backend and frontend
mise run check-large-files
```

Expected output: **No files listed** (all files under 400 lines)

## Common Issues and Solutions

### Issue: Circular Dependencies

**Problem:** Split files have circular import dependencies.

**Solution:** 
- Extract shared types to a common module
- Use trait objects or generics to break cycles
- Reorganize module hierarchy

### Issue: Private Type Visibility

**Problem:** Types that were private in a single file need to be shared across split files.

**Solution:**
- Use `pub(super)` for parent-module visibility
- Use `pub(crate)` for crate-wide visibility
- Re-export through `mod.rs`

### Issue: Unused Import Warnings

**Problem:** After splitting, some imports become unused.

**Solution:**
- Remove unused imports
- Run `cargo check` to identify them
- Use `cargo fix --bin "rhd" -p rhd_app` to auto-fix

## Example: Completed Splits

The following files have been successfully split following this skill:

| Original File | Lines | Split Into |
|---------------|-------|------------|
| `packages/rhd_api/src/lib.rs` | 593 | `execution.rs`, `chat.rs`, `ws.rs` |
| `packages/rhd_chat/src/stream.rs` | 516 | `stream/mod.rs`, `contract.rs`, `send.rs` |
| `packages/rhd_app/src/scenario/ai_chat.rs` | 475 | `ai_chat/mod.rs`, `simple.rs`, `mcp.rs`, `utils.rs` |
| `packages/rhd_app/src/execution.rs` | 410 | `execution/mod.rs`, `tracker.rs`, `handle.rs` |

## Checklist

When splitting a file, ensure:

- [ ] File size is under 400 lines after split
- [ ] All imports are correctly updated
- [ ] Public API remains unchanged
- [ ] `cargo check` passes without errors
- [ ] `cargo test --workspace` passes
- [ ] No unused import warnings
- [ ] Test files also under 400 lines
- [ ] Code is logically organized by responsibility
