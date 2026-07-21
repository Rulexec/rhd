# Templates Extraction and Restructure Plan

## Overview

Extract hardcoded text (tool definitions, role prompts) from the codebase into templates, and restructure the `templates/` folder for better organization.

## Current State

### Existing Templates
```
templates/
├── environment_details_no_role.md
├── environment_details_with_role.md
├── rhd_set_todo_list_contract.md
├── todo_list_empty.md
└── todo_list_with_items.md
```

### Hardcoded Text Locations

1. **Tool Definitions** (`packages/rhd_chat/src/tools.rs`):
   - `rhd_set_todo_list_tool_definition()` (lines 21-39)
   - `rhd_set_role_tool_definition()` (lines 41-59)

2. **Builtin Tool Definitions** (`packages/rhd_mcp_client/src/builtin.rs`):
   - `rhd_set_flag` tool definition (lines 23-42)

3. **Role Injection Prompts** (`packages/rhd_chat/src/projects.rs`):
   - `inject_roles_prompt()` (lines 228-241): Hardcoded text about roles list
   - `inject_role_system_prompt()` (lines 262-265): Hardcoded text about role switching

## Proposed New Structure

```
templates/
├── mcp_internal/
│   ├── rhd_set_todo_list/
│   │   ├── tool_definition.json    # Tool definition JSON
│   │   └── contract.md             # Existing contract (moved from root)
│   ├── rhd_set_role/
│   │   └── tool_definition.json    # Tool definition JSON
│   └── rhd_set_flag/
│       └── tool_definition.json    # Tool definition JSON
├── environment/
│   ├── details_no_role.md          # Moved from root
│   ├── details_with_role.md        # Moved from root
│   ├── todo_list_empty.md          # Moved from root
│   └── todo_list_with_items.md     # Moved from root
├── roles/
│   ├── roles_list_prompt.md        # Roles list injection template
│   └── role_switch_prompt.md       # Role switch notification template
```

## Implementation Steps

### Step 1: Create New Folder Structure
- Create `templates/mcp_internal/rhd_set_todo_list/`
- Create `templates/mcp_internal/rhd_set_role/`
- Create `templates/mcp_internal/rhd_set_flag/`
- Create `templates/environment/`
- Create `templates/roles/`

### Step 2: Move Existing Files to Appropriate Folders

| From | To |
|------|-----|
| `templates/rhd_set_todo_list_contract.md` | `templates/mcp_internal/rhd_set_todo_list/contract.md` |
| `templates/environment_details_no_role.md` | `templates/environment/details_no_role.md` |
| `templates/environment_details_with_role.md` | `templates/environment/details_with_role.md` |
| `templates/todo_list_empty.md` | `templates/environment/todo_list_empty.md` |
| `templates/todo_list_with_items.md` | `templates/environment/todo_list_with_items.md` |

### Step 3: Extract Tool Definitions

#### 3.1 rhd_set_todo_list
- Create `mcp_internal/rhd_set_todo_list/tool_definition.json` with the tool definition JSON

#### 3.2 rhd_set_role
- Create `mcp_internal/rhd_set_role/tool_definition.json` with the tool definition JSON

#### 3.3 rhd_set_flag
- Create `mcp_internal/rhd_set_flag/tool_definition.json` with the tool definition JSON

### Step 4: Extract Role Prompts

#### 4.1 roles_list_prompt.md
Template for the roles list injection (from `inject_roles_prompt()`):
```markdown
Your behavior is defined by the current active role. Current active role is "{currentRoleName}". You can switch your role by tool `rhd_set_role`.

These are the currently available roles:

{rolesList}
```

#### 4.2 role_switch_prompt.md
Template for role switch notification (from `inject_role_system_prompt()`):
```markdown
Your current role is now "{roleName}".

-----

{systemPrompt}
```

### Step 5: Update TemplateRegistry

Update `packages/rhd_app/src/templates.rs` to:
- Load templates from the new folder structure
- Use a naming convention like `mcp_internal/rhd_set_todo_list/tool_definition` for nested templates

### Step 6: Update Code to Use Templates

#### 6.1 Update `packages/rhd_chat/src/tools.rs`
- Modify `rhd_set_todo_list_tool_definition()` to load from template
- Modify `rhd_set_role_tool_definition()` to load from template

#### 6.2 Update `packages/rhd_mcp_client/src/builtin.rs`
- Modify `list_tool_definitions()` to load from template

#### 6.3 Update `packages/rhd_chat/src/projects.rs`
- Modify `inject_roles_prompt()` to use `roles/roles_list_prompt` template
- Modify `inject_role_system_prompt()` to use `roles/role_switch_prompt` template

### Step 7: Update Documentation

Update `memory/features/templates.md` to reflect:
- New folder structure
- New template names
- How to add new MCP internal tool templates
- How to add new role templates

## Template Naming Convention

| Template Path | Purpose |
|---------------|---------|
| `mcp_internal/rhd_set_todo_list/tool_definition` | Tool definition JSON for rhd_set_todo_list |
| `mcp_internal/rhd_set_todo_list/contract` | Contract documentation for rhd_set_todo_list |
| `mcp_internal/rhd_set_role/tool_definition` | Tool definition JSON for rhd_set_role |
| `mcp_internal/rhd_set_flag/tool_definition` | Tool definition JSON for rhd_set_flag |
| `environment/details_no_role` | Environment details without role |
| `environment/details_with_role` | Environment details with role |
| `environment/todo_list_empty` | Empty todo list prompt |
| `environment/todo_list_with_items` | Todo list with items template |
| `roles/roles_list_prompt` | Roles list injection template |
| `roles/role_switch_prompt` | Role switch notification template |

## Files to Modify

1. `packages/rhd_app/src/templates.rs` - Update template loading
2. `packages/rhd_chat/src/tools.rs` - Use templates for tool definitions
3. `packages/rhd_mcp_client/src/builtin.rs` - Use template for tool definition
4. `packages/rhd_chat/src/projects.rs` - Use templates for role prompts
5. `memory/features/templates.md` - Update documentation

## Files to Create

1. `templates/mcp_internal/rhd_set_todo_list/tool_definition.json`
2. `templates/mcp_internal/rhd_set_role/tool_definition.json`
3. `templates/mcp_internal/rhd_set_flag/tool_definition.json`
4. `templates/roles/roles_list_prompt.md`
5. `templates/roles/role_switch_prompt.md`

## Files to Move

1. `templates/rhd_set_todo_list_contract.md` → `templates/mcp_internal/rhd_set_todo_list/contract.md`
2. `templates/environment_details_no_role.md` → `templates/environment/details_no_role.md`
3. `templates/environment_details_with_role.md` → `templates/environment/details_with_role.md`
4. `templates/todo_list_empty.md` → `templates/environment/todo_list_empty.md`
5. `templates/todo_list_with_items.md` → `templates/environment/todo_list_with_items.md`
