# Templates Feature

## Overview

Templates are markdown and JSON files used for rendering dynamic content in the RHD system. They are embedded into the binary at compile time using Rust's `include_str!` macro, eliminating runtime file I/O and path resolution issues.

## Architecture

### Compile-Time Template Registry

Templates are loaded at compile time via the `TemplateRegistry` in [`packages/rhd_app/src/templates.rs`](../../../packages/rhd_app/src/templates.rs). This approach provides:

- **No runtime dependencies**: Templates are embedded in the binary
- **Faster startup**: No file I/O at initialization
- **Simpler deployment**: Single binary contains everything
- **Type safety**: Compile-time errors if template files are missing
- **Better performance**: Templates are in static memory, no heap allocation for storage

### TemplateLoader

The [`TemplateLoader`](../../../packages/rhd_app/src/template_loader.rs) provides a simple interface for accessing and rendering templates:

```rust
pub struct TemplateLoader;

impl TemplateLoader {
    pub fn new() -> Self;
    pub fn get_template(&self, name: &str) -> Option<&'static str>;
}
```

## Folder Structure

Templates are organized into logical folders based on their purpose:

```
templates/
├── mcp_internal/           # MCP tool definitions and contracts
│   ├── rhd_set_todo_list/
│   │   ├── tool_definition.json
│   │   └── contract.md
│   ├── rhd_set_role/
│   │   └── tool_definition.json
│   └── rhd_set_flag/
│       └── tool_definition.json
├── environment/            # Environment details and todo list templates
│   ├── details_no_role.md
│   ├── details_with_role.md
│   ├── todo_list_empty.md
│   └── todo_list_with_items.md
└── roles/                  # Role-related prompts
    ├── roles_list_prompt.md
    └── role_switch_prompt.md
```

## Available Templates

### MCP Internal Tools

| Template Path | Purpose |
|---------------|---------|
| `mcp_internal/rhd_set_todo_list/tool_definition` | Tool definition JSON for rhd_set_todo_list |
| `mcp_internal/rhd_set_todo_list/contract` | Contract documentation for rhd_set_todo_list |
| `mcp_internal/rhd_set_role/tool_definition` | Tool definition JSON for rhd_set_role |
| `mcp_internal/rhd_set_flag/tool_definition` | Tool definition JSON for rhd_set_flag |

### Environment Templates

| Template Path | Purpose |
|---------------|---------|
| `environment/details_no_role` | Environment details without active role |
| `environment/details_with_role` | Environment details with active role |
| `environment/todo_list_empty` | Empty todo list prompt |
| `environment/todo_list_with_items` | Todo list with items template |

### Role Templates

| Template Path | Purpose |
|---------------|---------|
| `roles/roles_list_prompt` | Roles list injection template |
| `roles/role_switch_prompt` | Role switch notification template |

## Adding New Templates

To add a new template:

1. Create a new file in the appropriate folder:
   - MCP tool definitions: `templates/mcp_internal/<tool_name>/`
   - Environment templates: `templates/environment/`
   - Role templates: `templates/roles/`

2. Add the template to the `TemplateRegistry::get()` match statement in [`packages/rhd_app/src/templates.rs`](../../../packages/rhd_app/src/templates.rs)

3. Rebuild the project to embed the new template

## Placeholder Syntax

Templates support placeholder substitution using the `{placeholderName}` syntax. Placeholders are replaced with values during rendering.

Example template:
```markdown
# Todo List

{todoItems}

Current Role: {currentRoleName}
```

## Usage

Templates are used throughout the application for:

- **Tool Definitions**: Providing AI models with tool usage instructions (JSON format)
- **Environment Details**: Injecting system context into AI prompts
- **Todo List Rendering**: Displaying todo items in a formatted table
- **Role Prompts**: Managing role switching and role list injection

The `TemplateLoader` is initialized in the daemon and passed to components that need template access via `Arc<TemplateLoader>`.
