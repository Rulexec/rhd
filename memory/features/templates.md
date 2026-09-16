# Templates Feature

## Overview

Templates are markdown and JSON files in the shared top-level `templates/` directory, used for tool definitions, contracts, and prompt rendering. Each consumer plugin embeds the whole directory at compile time with `include_dir!` (`$CARGO_MANIFEST_DIR/../../templates`) and loads the specific files it needs — no runtime file I/O or path resolution.

## Architecture

- **Compile-time embedding**: `include_dir!` embeds `templates/` into the plugin binary; missing files fail at compile time.
- **Per-plugin loaders**: there is no central TemplateLoader. Each plugin has its own `templates.rs`:
  - `plugins/rhd_plugin_todo_list/src/templates.rs` — `Templates::load()` + typed accessors, renders via `{placeholder}` string replacement
  - `plugins/rhd_plugin_choice/src/templates.rs` — loads the `rhd_choice` tool definition
- **Template names** are the relative paths inside `templates/` (e.g. `mcp_internal/rhd_set_todo_list/tool_definition.json`).

## Folder Structure & Consumers

```
templates/
├── mcp_internal/                   # Tool definitions and contracts
│   ├── rhd_choice/                 # → used by rhd_plugin_choice
│   ├── rhd_set_todo_list/          # → used by rhd_plugin_todo_list
│   │   ├── tool_definition.json
│   │   ├── contract.md
│   │   └── tool_error_invalid_format.md
│   ├── rhd_set_flag/               # ⚠ legacy vestige (scenario-era, unused)
│   └── rhd_set_role/               # ⚠ legacy vestige (unused)
└── environment/                    # Prompt injection templates
    ├── todo_list_empty.md          # → used by rhd_plugin_todo_list
    ├── todo_list_with_items.md     # → used by rhd_plugin_todo_list
    ├── details_no_role.md          # ⚠ legacy vestige (unused)
    └── details_with_role.md        # ⚠ legacy vestige (unused)
```

`templates/roles/` (`roles_list_prompt.md`, `role_switch_prompt.md`) is also a legacy vestige with no code consumers.

## Adding New Templates

1. Create the file under `templates/` in the appropriate folder
2. Add the path constant + load + accessor in the consuming plugin's `templates.rs`
3. Rebuild — `include_dir!` picks up the new file automatically

## Placeholder Syntax

Rendering is simple `String::replace` of `{placeholderName}` tokens (e.g., `{todoItems}` in `todo_list_with_items.md`).

## Usage

- **Tool definitions**: JSON schemas registered by plugins so AI models can call their tools
- **Contracts**: system-message documentation injected into tagged chats (todo list)
- **Todo list rendering**: formatted task tables injected into AI context
