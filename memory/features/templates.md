# Templates Feature

## Overview

Templates are markdown files used for rendering dynamic content in the RHD system. They are embedded into the binary at compile time using Rust's `include_str!` macro, eliminating runtime file I/O and path resolution issues.

## Architecture

### Compile-Time Template Registry

Templates are loaded at compile time via the `TemplateRegistry` in [`packages/rhd_app/src/templates.rs`](../../packages/rhd_app/src/templates.rs). This approach provides:

- **No runtime dependencies**: Templates are embedded in the binary
- **Faster startup**: No file I/O at initialization
- **Simpler deployment**: Single binary contains everything
- **Type safety**: Compile-time errors if template files are missing
- **Better performance**: Templates are in static memory, no heap allocation for storage

### TemplateLoader

The [`TemplateLoader`](../../packages/rhd_app/src/template_loader.rs) provides a simple interface for accessing and rendering templates:

```rust
pub struct TemplateLoader;

impl TemplateLoader {
    pub fn new() -> Self;
    pub fn get_template(&self, name: &str) -> Option<&'static str>;
    pub fn render_template(&self, name: &str, replacements: &HashMap<String, String>) -> Option<String>;
}
```

## Available Templates

| Template Name | Purpose |
|---------------|---------|
| `environment_details_no_role` | Environment details without active role |
| `environment_details_with_role` | Environment details with active role |
| `rhd_set_todo_list_contract` | Tool contract for todo list |
| `todo_list_empty` | Empty todo list prompt |
| `todo_list_with_items` | Todo list with items template |

## Template Location

Template files are stored in the `templates/` directory at the project root:

```
templates/
├── environment_details_no_role.md
├── environment_details_with_role.md
├── rhd_set_todo_list_contract.md
├── todo_list_empty.md
└── todo_list_with_items.md
```

## Adding New Templates

To add a new template:

1. Create a new `.md` file in the `templates/` directory
2. Add the template to the `TemplateRegistry::get()` match statement in [`packages/rhd_app/src/templates.rs`](../../packages/rhd_app/src/templates.rs)
3. Add the template name to the `list_templates()` function
4. Rebuild the project to embed the new template

## Placeholder Syntax

Templates support placeholder substitution using the `{placeholderName}` syntax. Placeholders are replaced with values from a `HashMap<String, String>` during rendering.

Example template:
```markdown
# Todo List

{todoItems}

Current Role: {currentRoleName}
```

## Usage

Templates are used throughout the application for:

- **Environment Details**: Injecting system context into AI prompts
- **Todo List Rendering**: Displaying todo items in a formatted table
- **Tool Contracts**: Providing AI models with tool usage instructions

The `TemplateLoader` is initialized in the daemon and passed to components that need template access via `Arc<TemplateLoader>`.
