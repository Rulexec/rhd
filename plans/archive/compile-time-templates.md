# Compile-Time Template Loading

## Problem Statement

The current `TemplateLoader` implementation reads template files from the filesystem at runtime using `std::env::current_dir()?.join("templates")`. This approach has several issues:

1. **Runtime dependency**: Templates must be present in the filesystem when the binary runs
2. **Path resolution issues**: The path is relative to the current working directory, which may not be the project root when the binary is executed
3. **Deployment complexity**: Templates need to be distributed alongside the binary
4. **Performance**: File I/O at startup adds latency

## Current Implementation

**File**: `packages/rhd_app/src/template_loader.rs`

```rust
pub struct TemplateLoader {
    templates: HashMap<String, String>,
}

impl TemplateLoader {
    pub fn new(templates_dir: &Path) -> Result<Self, TemplateLoadError> {
        // Reads .md files from directory at runtime
        // Stores in HashMap<String, String>
    }
    
    pub fn get_template(&self, name: &str) -> Option<&String> {
        self.templates.get(name)
    }
}
```

**Usage in daemon.rs**:
```rust
let templates_dir = std::env::current_dir()?.join("templates");
let template_loader = Arc::new(TemplateLoader::new(&templates_dir)?);
```

## Current Templates

The following templates exist in the `templates/` directory:

1. `environment_details_no_role.md` - Environment details without active role
2. `environment_details_with_role.md` - Environment details with active role
3. `rhd_set_todo_list_contract.md` - Tool contract for todo list
4. `todo_list_empty.md` - Empty todo list prompt
5. `todo_list_with_items.md` - Todo list with items template

## Solution: Compile-Time Template Embedding

### Approach

Use Rust's `include_str!` macro to embed template content directly into the binary at compile time. This eliminates runtime file I/O and path resolution issues.

### Implementation Plan

#### Phase 1: Create Template Registry Module

**File**: `packages/rhd_app/src/templates.rs` (new file)

```rust
/// Compile-time template registry
/// Templates are embedded into the binary at compile time using include_str!

pub struct TemplateRegistry;

impl TemplateRegistry {
    /// Get a template by name
    pub fn get(name: &str) -> Option<&'static str> {
        match name {
            "environment_details_no_role" => Some(include_str!("../../templates/environment_details_no_role.md")),
            "environment_details_with_role" => Some(include_str!("../../templates/environment_details_with_role.md")),
            "rhd_set_todo_list_contract" => Some(include_str!("../../templates/rhd_set_todo_list_contract.md")),
            "todo_list_empty" => Some(include_str!("../../templates/todo_list_empty.md")),
            "todo_list_with_items" => Some(include_str!("../../templates/todo_list_with_items.md")),
            _ => None,
        }
    }
    
    /// Get all available template names
    pub fn list_templates() -> Vec<&'static str> {
        vec![
            "environment_details_no_role",
            "environment_details_with_role",
            "rhd_set_todo_list_contract",
            "todo_list_empty",
            "todo_list_with_items",
        ]
    }
}
```

#### Phase 2: Refactor TemplateLoader

**File**: `packages/rhd_app/src/template_loader.rs`

Replace the runtime file loading with compile-time registry:

```rust
use std::collections::HashMap;
use crate::templates::TemplateRegistry;

#[derive(Debug, Error)]
pub enum TemplateLoadError {
    #[error("template not found: {0}")]
    NotFound(String),
}

pub struct TemplateLoader {
    // No longer need HashMap, templates are in static memory
}

impl TemplateLoader {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn get_template(&self, name: &str) -> Option<&'static str> {
        TemplateRegistry::get(name)
    }
    
    pub fn render_template(
        &self,
        name: &str,
        replacements: &HashMap<String, String>,
    ) -> Option<String> {
        TemplateRegistry::get(name).map(|template| {
            let mut result = template.to_string();
            for (key, value) in replacements {
                result = result.replace(&format!("{{{}}}", key), value);
            }
            result
        })
    }
}
```

#### Phase 3: Update Daemon Initialization

**File**: `packages/rhd_app/src/daemon.rs`

Remove the templates directory path resolution:

```rust
// Before:
let templates_dir = std::env::current_dir()?.join("templates");
let template_loader = Arc::new(TemplateLoader::new(&templates_dir)?);

// After:
let template_loader = Arc::new(TemplateLoader::new());
```

#### Phase 4: Update TemplateLoaderRef Usage

**File**: `packages/rhd_chat/src/stream.rs`

The `TemplateLoaderRef` already uses a closure-based approach, so it should work with the new implementation. Update the closure to use the new API:

```rust
let template_loader_ref = rhd_chat::stream::TemplateLoaderRef::new(move |name| {
    template_loader.get_template(name).map(|s| s.to_string())
});
```

#### Phase 5: Update Tests

**File**: `packages/rhd_app/src/template_loader.rs`

Remove tests that rely on runtime file loading:
- `test_load_templates_from_directory`
- `test_load_templates_nonexistent_directory`
- `test_load_templates_skips_non_md_files`

Add new tests for compile-time registry:
- `test_get_existing_template`
- `test_get_nonexistent_template`
- `test_list_templates`
- `test_render_template_with_replacements`

#### Phase 6: Update Documentation

**File**: `memory/features/templates.md` (if exists) or create new

Document the compile-time template system:
- How to add new templates
- Template naming conventions
- Placeholder syntax

### Benefits

1. **No runtime dependencies**: Templates are embedded in the binary
2. **Faster startup**: No file I/O at initialization
3. **Simpler deployment**: Single binary contains everything
4. **Type safety**: Compile-time errors if template files are missing
5. **Better performance**: Templates are in static memory, no heap allocation for storage

### Migration Steps

1. Create `packages/rhd_app/src/templates.rs` with `TemplateRegistry`
2. Update `packages/rhd_app/src/template_loader.rs` to use `TemplateRegistry`
3. Update `packages/rhd_app/src/daemon.rs` to remove path resolution
4. Update `packages/rhd_chat/src/stream.rs` to use new API
5. Update tests in `template_loader.rs`
6. Run `cargo build` to verify compilation
7. Run `cargo test` to verify functionality
8. Test the application manually to ensure templates load correctly

### Testing Strategy

1. **Unit tests**: Verify `TemplateRegistry::get()` returns correct content
2. **Integration tests**: Verify `TemplateLoader` works with new implementation
3. **Manual testing**: 
   - Start the daemon
   - Create a new chat
   - Verify todo contract is injected
   - Verify environment details are rendered correctly

### Future Enhancements

1. **Template validation**: Add compile-time validation of placeholder syntax
2. **Template versioning**: Track template versions for migration purposes
3. **Template compression**: Consider compressing large templates to reduce binary size
4. **Hot reloading**: For development, optionally support runtime template loading

## Implementation Checklist

- [ ] Create `packages/rhd_app/src/templates.rs`
- [ ] Implement `TemplateRegistry` with `get()` and `list_templates()`
- [ ] Refactor `TemplateLoader` to use `TemplateRegistry`
- [ ] Update `daemon.rs` initialization
- [ ] Update `stream.rs` template loader reference
- [ ] Remove old runtime loading tests
- [ ] Add new compile-time registry tests
- [ ] Update documentation
- [ ] Run `cargo build`
- [ ] Run `cargo test`
- [ ] Manual testing

## Rollback Plan

If issues arise, the old implementation can be restored by:
1. Reverting changes to `template_loader.rs`
2. Reverting changes to `daemon.rs`
3. Removing `templates.rs`
4. Ensuring templates directory is present at runtime

## Notes

- The `include_str!` macro paths are relative to the source file location
- Template content is stored in the binary's read-only data section
- No runtime memory allocation for template storage
- Template names must match exactly (case-sensitive)
