# Phase 2: Templates Folder & Message Templates

## Overview

This phase creates the templates folder structure and defines message templates for todo list injection into the AI conversation. Templates allow easy modification of prompts without code changes.

## Files to Create

### 1. `templates/` Directory Structure

Create the following directory structure in the project root:

```
templates/
├── environment_details_with_role.md
├── environment_details_no_role.md
├── todo_list_empty.md
├── todo_list_with_items.md
└── rhd_set_todo_list_contract.md
```

### 2. `templates/environment_details_with_role.md`

Template for environment details when a role is active:

```markdown
<environment_details>
# Current role
<name>{currentRoleName}</name>

# TODO list

Below is your current list of reminders for this task. Keep them updated as you progress with `rhd_set_todo_list`.

| # | Content | Status |
|---|---------|--------|
{todoItems}
</environment_details>
```

**Placeholders:**
- `{currentRoleName}` - The name of the currently active role
- `{todoItems}` - Rendered todo list items (one per line in table format)

### 3. `templates/environment_details_no_role.md`

Template for environment details when no role is active:

```markdown
<environment_details>
# TODO list

Below is your current list of reminders for this task. Keep them updated as you progress with `rhd_set_todo_list`.

| # | Content | Status |
|---|---------|--------|
{todoItems}
</environment_details>
```

**Placeholders:**
- `{todoItems}` - Rendered todo list items (one per line in table format)

### 4. `templates/todo_list_empty.md`

Template for when no todo list exists:

```markdown
You have not created a todo list yet. Create one with `update_todo_list` if your task is complicated or involves multiple steps.
```

### 5. `templates/todo_list_with_items.md`

Template for rendering todo list items as a table:

```markdown
| # | Content | Status |
|---|---------|--------|
{todoItems}
```

**Note:** This template is used internally by the environment_details templates. The `{todoItems}` placeholder is replaced with rendered rows.

### 6. `templates/rhd_set_todo_list_contract.md`

System message template for tool contract (content from `plans/assets/todo.md`):

```markdown
# rhd_set_todo_list Tool Contract

## Overview

The `rhd_set_todo_list` tool manages a task tracking list for multi-step operations. It replaces the entire todo list with a new one provided as a markdown-formatted checklist.

## Tool Definition

```json
{
  "name": "rhd_set_todo_list",
  "description": "Replace the entire TODO list with an updated checklist reflecting the current state. Always provide the full list; the system will overwrite the previous one. This tool is designed for step-by-step task tracking, allowing you to confirm completion of each step before updating, update multiple statuses at once (e.g., mark one as completed and start the next), and dynamically add new todos as they're discovered.",
  "parameters": {
    "type": "object",
    "properties": {
      "todos": {
        "type": "string",
        "description": "Full markdown checklist in execution order, using [ ] for pending, [x] for completed, [-] for in progress, and [!] for discarded"
      }
    },
    "required": ["todos"]
  }
}
```

## Input Format

The `todos` parameter is a **single string** containing a markdown checklist with the following syntax:

### Checkbox States

| Syntax | Status | Description |
|--------|--------|-------------|
| `[ ]` | Pending | Task not yet started |
| `[-]` | In Progress | Task currently being worked on |
| `[x]` | Completed | Task finished |
| `[!]` | Discarded | Task no longer needed (strikethrough in UI) |

### Format Rules

1. **One item per line** - Each todo item must be on its own line
2. **Execution order** - Items should be listed in the order they will be executed
3. **Single-level list** - No nesting or subtasks allowed
4. **Full replacement** - Every call must include the complete list; partial updates are not supported

### Example Input

```
[x] Analyze the issue and design the fix
[x] Create the fix plan document
[-] Get user approval
[ ] Implement: Add database column
[ ] Implement: Modify set_active_role()
[ ] Implement: Add inject_pending_role_prompt() function
[ ] Verify: Build and test
```

## Output Format

The system renders the todo list as a formatted table in the environment details:

```
| # | Content | Status |
|---|---------|--------|
| 1 | Analyze the issue and design the fix | Completed |
| 2 | Create the fix plan document | Completed |
| 3 | Get user approval | In Progress |
| 4 | Implement: Add database column | Pending |
| 5 | Implement: Modify set_active_role() | Pending |
| 6 | Implement: Add inject_pending_role_prompt() function | Pending |
| 7 | Verify: Build and test | Pending |
```

### Status Mapping

| Input Syntax | Rendered Status |
|--------------|-----------------|
| `[ ]` | Pending |
| `[-]` | In Progress |
| `[x]` | Completed |
| `[!]` | Discarded |

## Usage Guidelines

### When to Use

- Task involves multiple steps or requires ongoing tracking
- Need to update status of several items at once
- New actionable items are discovered during execution
- Task is complex and benefits from stepwise progress tracking

### When NOT to Use

- Only a single, trivial task
- Task can be completed in one or two simple steps
- Request is purely conversational or informational

### Best Practices

1. **Update frequently** - Call the tool after completing each significant step
2. **Mark completion before starting next** - Change current item to `[x]` and next item to `[-]`
3. **Add new items dynamically** - If new tasks are discovered, append them to the list
4. **Keep all unfinished tasks** - Don't remove tasks unless explicitly completed or instructed
5. **One item per logical step** - Granularity should match meaningful progress checkpoints

### Example Workflow

**Initial call:**
```
[-] Analyze requirements
[ ] Design solution
[ ] Implement changes
[ ] Test implementation
```

**After analysis complete:**
```
[x] Analyze requirements
[-] Design solution
[ ] Implement changes
[ ] Test implementation
```

**After design complete, discovered new subtask:**
```
[x] Analyze requirements
[x] Design solution
[-] Implement changes
[ ] Add database migration
[ ] Test implementation
```

**Final state:**
```
[x] Analyze requirements
[x] Design solution
[x] Implement changes
[x] Add database migration
[x] Test implementation
```

## Response Format

On success, the tool returns:
```
Todo list updated successfully.
```

## Implementation Notes

1. The tool completely replaces the previous list - there is no merge or partial update
2. Items are automatically numbered in the rendered output (1, 2, 3, ...)
3. The system reminds the agent to update the todo list when task status changes
4. Only one todo list exists per conversation session
5. The list persists across tool calls within the same session
```

## Files to Modify

### 1. `packages/rhd_app/src/project_loader.rs` (or new file `packages/rhd_app/src/template_loader.rs`)

**Add template loading functionality:**

```rust
use std::path::Path;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateLoadError {
    #[error("failed to read template file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

pub struct TemplateLoader {
    templates: HashMap<String, String>,
}

impl TemplateLoader {
    pub fn new(templates_dir: &Path) -> Result<Self, TemplateLoadError> {
        let mut templates = HashMap::new();
        
        if !templates_dir.exists() {
            return Ok(Self { templates });
        }
        
        let entries = templates_dir.read_dir().map_err(|source| TemplateLoadError::Io {
            path: templates_dir.display().to_string(),
            source,
        })?;
        
        for entry in entries {
            let entry = entry.map_err(|source| TemplateLoadError::Io {
                path: templates_dir.display().to_string(),
                source,
            })?;
            let path = entry.path();
            
            if !path.is_file() {
                continue;
            }
            
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    let template_name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let content = std::fs::read_to_string(&path).map_err(|source| TemplateLoadError::Io {
                        path: path.display().to_string(),
                        source,
                    })?;
                    
                    templates.insert(template_name, content);
                }
            }
        }
        
        Ok(Self { templates })
    }
    
    pub fn get_template(&self, name: &str) -> Option<&String> {
        self.templates.get(name)
    }
    
    pub fn render_template(&self, name: &str, replacements: &HashMap<String, String>) -> Option<String> {
        self.templates.get(name).map(|template| {
            let mut result = template.clone();
            for (key, value) in replacements {
                result = result.replace(&format!("{{{}}}", key), value);
            }
            result
        })
    }
}
```

### 2. `packages/rhd_app/src/daemon.rs`

**Add template loader to DaemonState:**

```rust
use crate::template_loader::TemplateLoader;

pub struct DaemonState {
    // ... existing fields ...
    pub template_loader: Arc<TemplateLoader>,
}

impl DaemonState {
    pub fn new(/* ... existing params ... */) -> Result<Self, Box<dyn std::error::Error>> {
        // ... existing initialization ...
        
        let templates_dir = std::env::current_dir()?.join("templates");
        let template_loader = Arc::new(TemplateLoader::new(&templates_dir)?);
        
        Ok(Self {
            // ... existing fields ...
            template_loader,
        })
    }
}
```

## Template Rendering Logic

### Todo List Item Rendering

```rust
pub fn render_todo_items(items: &[crate::TodoItem]) -> String {
    let mut output = String::new();
    
    for (idx, item) in items.iter().enumerate() {
        let status_str = match item.status {
            crate::TodoStatus::Pending => "Pending",
            crate::TodoStatus::InProgress => "In Progress",
            crate::TodoStatus::Completed => "Completed",
            crate::TodoStatus::Discarded => "Discarded",
        };
        
        output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, status_str));
    }
    
    output
}
```

### Environment Details Rendering

```rust
pub fn render_environment_details(
    template_loader: &TemplateLoader,
    todo_items: &[crate::TodoItem],
    active_role: Option<&(String, String)>,
) -> String {
    let todo_items_str = if todo_items.is_empty() {
        template_loader.get_template("todo_list_empty")
            .map(|s| s.as_str())
            .unwrap_or("No todo list created yet.")
            .to_string()
    } else {
        let rendered_items = render_todo_items(todo_items);
        let mut replacements = HashMap::new();
        replacements.insert("todoItems".to_string(), rendered_items);
        
        template_loader.render_template("todo_list_with_items", &replacements)
            .unwrap_or_else(|| "Failed to render todo list.".to_string())
    };
    
    let template_name = if active_role.is_some() {
        "environment_details_with_role"
    } else {
        "environment_details_no_role"
    };
    
    let mut replacements = HashMap::new();
    replacements.insert("todoItems".to_string(), todo_items_str);
    
    if let Some((project_name, role_name)) = active_role {
        replacements.insert("currentRoleName".to_string(), format!("{} ({})", role_name, project_name));
    }
    
    template_loader.render_template(template_name, &replacements)
        .unwrap_or_else(|| "Failed to render environment details.".to_string())
}
```

## Tests

### Template Loading Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_load_templates_from_directory() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();
        
        fs::write(
            templates_dir.join("test_template.md"),
            "Hello {name}!",
        ).unwrap();
        
        let loader = TemplateLoader::new(templates_dir).unwrap();
        assert!(loader.get_template("test_template").is_some());
        assert_eq!(loader.get_template("test_template").unwrap(), "Hello {name}!");
    }

    #[test]
    fn test_render_template_with_replacements() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();
        
        fs::write(
            templates_dir.join("greeting.md"),
            "Hello {name}, welcome to {place}!",
        ).unwrap();
        
        let loader = TemplateLoader::new(templates_dir).unwrap();
        
        let mut replacements = HashMap::new();
        replacements.insert("name".to_string(), "Alice".to_string());
        replacements.insert("place".to_string(), "Wonderland".to_string());
        
        let rendered = loader.render_template("greeting", &replacements).unwrap();
        assert_eq!(rendered, "Hello Alice, welcome to Wonderland!");
    }

    #[test]
    fn test_render_template_missing_placeholder() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();
        
        fs::write(
            templates_dir.join("partial.md"),
            "Hello {name}!",
        ).unwrap();
        
        let loader = TemplateLoader::new(templates_dir).unwrap();
        
        let mut replacements = HashMap::new();
        replacements.insert("name".to_string(), "Bob".to_string());
        // Missing "place" placeholder
        
        let rendered = loader.render_template("partial", &replacements).unwrap();
        assert_eq!(rendered, "Hello Bob!");
    }

    #[test]
    fn test_load_templates_nonexistent_directory() {
        let loader = TemplateLoader::new(Path::new("/nonexistent/path")).unwrap();
        assert!(loader.get_template("any_template").is_none());
    }

    #[test]
    fn test_load_templates_skips_non_md_files() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();
        
        fs::write(templates_dir.join("valid.md"), "Valid template").unwrap();
        fs::write(templates_dir.join("invalid.txt"), "Invalid file").unwrap();
        fs::write(templates_dir.join("also_invalid.json"), "{}").unwrap();
        
        let loader = TemplateLoader::new(templates_dir).unwrap();
        assert!(loader.get_template("valid").is_some());
        assert!(loader.get_template("invalid").is_none());
        assert!(loader.get_template("also_invalid").is_none());
    }
}
```

### Todo List Rendering Tests

```rust
#[test]
fn test_render_todo_items_empty() {
    let items: Vec<crate::TodoItem> = vec![];
    let rendered = render_todo_items(&items);
    assert_eq!(rendered, "");
}

#[test]
fn test_render_todo_items_single() {
    let items = vec![
        crate::TodoItem {
            content: "Test task".to_string(),
            status: crate::TodoStatus::Pending,
        },
    ];
    let rendered = render_todo_items(&items);
    assert!(rendered.contains("| 1 | Test task | Pending |"));
}

#[test]
fn test_render_todo_items_multiple() {
    let items = vec![
        crate::TodoItem {
            content: "Task 1".to_string(),
            status: crate::TodoStatus::Completed,
        },
        crate::TodoItem {
            content: "Task 2".to_string(),
            status: crate::TodoStatus::InProgress,
        },
        crate::TodoItem {
            content: "Task 3".to_string(),
            status: crate::TodoStatus::Pending,
        },
    ];
    let rendered = render_todo_items(&items);
    assert!(rendered.contains("| 1 | Task 1 | Completed |"));
    assert!(rendered.contains("| 2 | Task 2 | In Progress |"));
    assert!(rendered.contains("| 3 | Task 3 | Pending |"));
}

#[test]
fn test_render_todo_items_with_discarded() {
    let items = vec![
        crate::TodoItem {
            content: "Active task".to_string(),
            status: crate::TodoStatus::InProgress,
        },
        crate::TodoItem {
            content: "Discarded task".to_string(),
            status: crate::TodoStatus::Discarded,
        },
    ];
    let rendered = render_todo_items(&items);
    assert!(rendered.contains("| 1 | Active task | In Progress |"));
    assert!(rendered.contains("| 2 | Discarded task | Discarded |"));
}
```

## Implementation Notes

1. **Template caching**: Templates are loaded once at daemon startup and cached in memory for fast access.

2. **Placeholder syntax**: Uses `{placeholderName}` syntax for easy identification and replacement.

3. **Missing placeholders**: If a placeholder is not provided in the replacements map, it remains unchanged in the output (no error).

4. **File extension filtering**: Only `.md` files are loaded as templates.

5. **Template naming**: Template names are derived from filenames without the `.md` extension.

6. **Error handling**: Template loading errors are logged but don't prevent daemon startup. Missing templates result in fallback messages.

## Dependencies

- This phase depends on Phase 1 (Tool Definition & Storage) for the `TodoItem` and `TodoStatus` types
- This phase must be completed before Phase 3 (Tool Loop Injection)
- Phase 7 (Contract Injection) uses the `rhd_set_todo_list_contract.md` template created here
