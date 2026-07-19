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
