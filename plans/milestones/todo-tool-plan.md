# rhd_set_todo_list Tool Implementation Plan

## Overview

This plan introduces an internal `rhd_set_todo_list` tool that allows the AI to manage a task tracking list during multi-step operations. The tool is not available for scenarios (internal chat use only).

The tool contract is defined in [`plans/assets/todo.md`](plans/assets/todo.md) and should be used as the basis for implementation.

## Feature Requirements

### Tool Behavior
- AI can create/replace the entire todo list with a markdown-formatted checklist
- Todo list persists per chat session
- After each tool loop iteration, the current todo list is injected into the conversation as a user message

### Frontend UI
- Button near the roles selector showing completed/pending/total count
- Clicking the button expands to show the full todo list with statuses
- Real-time updates as the AI modifies the list

### Templates
- `templates/` folder in project root for message templates
- Template for environment_details injection (with/without roles)
- Template for tool contract (injected as system message on chat start)

---

## Implementation Phases

### Phase 1: Backend - Tool Definition & Storage

**Goal**: Define the `rhd_set_todo_list` tool and add storage for todo lists.

**Files to modify**:
- [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)
  - Add `RHD_SET_TODO_LIST_TOOL_NAME` constant
  - Add `rhd_set_todo_list_tool_definition()` function
  - Add `handle_rhd_set_todo_list()` handler function
  - Update `collect_builtin_tools()` to include the new tool
  - Update `execute_tool_call()` to route to the handler

- [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)
  - Add `todo_list` column to `chats` table (TEXT, nullable)
  - Add `set_todo_list(chat_id, todo_list)` method
  - Add `get_todo_list(chat_id)` method
  - Add database migration for the new column

- [`packages/rhd_chat/src/lib.rs`](packages/rhd_chat/src/lib.rs)
  - Add `TodoItem` struct with `content`, `status` fields
  - Add `TodoList` type alias or struct

**Tests**:
- Unit tests for tool definition
- Database migration tests
- Tool handler tests

---

### Phase 2: Templates Folder & Message Templates

**Goal**: Create the templates folder and define message templates for todo list injection.

**Files to create**:
- `templates/` directory in project root
- `templates/environment_details_with_role.md` - Template for environment details with role
- `templates/environment_details_no_role.md` - Template for environment details without role
- `templates/todo_list_empty.md` - Template for when no todo list exists
- `templates/todo_list_with_items.md` - Template for rendering todo list as table
- `templates/rhd_set_todo_list_contract.md` - System message template for tool contract (content from [`plans/assets/todo.md`](plans/assets/todo.md))

**Template Format**:
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

**Tests**:
- Template loading tests
- Template rendering tests

---

### Phase 3: Backend - Todo List Injection in Tool Loop

**Goal**: Inject todo list status after each tool loop iteration.

**Files to modify**:
- [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)
  - Add `inject_todo_list_message()` function
  - Call injection after each tool loop iteration (after tool results are processed)
  - Load templates from `templates/` folder
  - Build environment_details message based on current role and todo list

- [`packages/rhd_app/src/project_loader.rs`](packages/rhd_app/src/project_loader.rs) or new file
  - Add `load_template()` function to read template files
  - Cache templates at daemon startup

**Injection Logic**:
1. After tool loop iteration completes (all tool results processed)
2. Check if todo list exists for this chat
3. If exists, build environment_details message using template
4. Add as user message to conversation (not persisted to DB, only sent to AI)
5. If no todo list, inject prompt to create one

**Tests**:
- Injection timing tests
- Template rendering tests
- Role-aware injection tests

---

### Phase 4: Backend - WebSocket Protocol

**Goal**: Add WebSocket events for todo list updates.

**Files to modify**:
- [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs)
  - Add `WsEvent::TodoListUpdated` variant
  - Add `TodoListData` DTO with items array

- [`packages/rhd_chat/src/event.rs`](packages/rhd_chat/src/event.rs)
  - Add `ChatEvent::TodoListUpdated` variant

- [`packages/rhd_app/src/ws.rs`](packages/rhd_app/src/ws.rs)
  - Handle `TodoListUpdated` event
  - Send to subscribed clients

- [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)
  - Emit `TodoListUpdated` event when tool is called successfully

**Event Format**:
```json
{
  "type": "event",
  "event": "todoListUpdated",
  "data": {
    "chatId": 123,
    "items": [
      { "content": "Analyze requirements", "status": "completed" },
      { "content": "Design solution", "status": "in_progress" },
      { "content": "Implement changes", "status": "pending" },
      { "content": "Old task no longer needed", "status": "discarded" }
    ]
  }
}
```

**Tests**:
- WebSocket event tests
- Event propagation tests

---

### Phase 5: Frontend - Todo List Stores & WebSocket Handling

**Goal**: Add frontend stores and WebSocket handling for todo list.

**Files to modify**:
- [`frontend/src/lib/types/ws.ts`](frontend/src/lib/types/ws.ts)
  - Add `TodoListUpdatedEventSchema`
  - Add to `WsEventSchema` union

- [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts)
  - Add `todoList` writable store
  - Add `hasTodoList` derived store
  - Add `todoListStats` derived store (completed/pending/total counts)

- [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)
  - Handle `todoListUpdated` event
  - Update `todoList` store

- [`frontend/src/lib/actions/types.ts`](frontend/src/lib/actions/types.ts)
  - Add `todoListUpdated` action type

- [`frontend/src/lib/actions/processors.ts`](frontend/src/lib/actions/processors.ts)
  - Add processor for `todoListUpdated`

**Tests**:
- Store tests
- WebSocket handling tests

---

### Phase 6: Frontend - Todo List UI Component

**Goal**: Add UI component for todo list display near role selector.

**Files to create**:
- [`frontend/src/lib/components/TodoListButton.svelte`](frontend/src/lib/components/TodoListButton.svelte)
  - Button showing completed/pending/total count
  - Expandable list on click
  - Visual indicators for status (checkmark, spinner, empty circle, strikethrough)

**Files to modify**:
- [`frontend/src/lib/components/RoleSelector.svelte`](frontend/src/lib/components/RoleSelector.svelte) or parent component
  - Add `TodoListButton` next to role selector
  - Show only when todo list exists

**UI Behavior**:
- Button shows "3/5" (completed/total) format
- Click expands dropdown with full list
- Each item shows status icon and content
- Auto-updates via WebSocket events

**Styling**:
- Match existing role selector styling
- Status colors: green (completed), blue (in_progress), gray (pending), red/strikethrough (discarded)

**Tests**:
- Component tests
- UI interaction tests

---

### Phase 7: System Message Contract Injection

**Goal**: Inject tool contract as system message on chat start.

**Files to modify**:
- [`packages/rhd_chat/src/stream.rs`](packages/rhd_chat/src/stream.rs)
  - Add `inject_todo_tool_contract()` function
  - Call on first message in chat (when no messages exist yet)
  - Load contract from `templates/rhd_set_todo_list_contract.md`

- [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)
  - Update `collect_builtin_tools()` to always include `rhd_set_todo_list`
  - (Tool is always available in chat, not conditional on roles)

**Contract Content**:
The contract template should include content from [`plans/assets/todo.md`](plans/assets/todo.md):
- Tool name and description
- Input format (markdown checklist)
- Checkbox states ([ ], [-], [x], [!])
- Format rules
- Example input/output
- Usage guidelines

**Tests**:
- Contract injection tests
- System message format tests

---

## Dependency Graph

```
Phase 1 (Tool Definition & Storage)
    ↓
Phase 2 (Templates Folder)
    ↓
Phase 3 (Tool Loop Injection)
    ↓
Phase 4 (WebSocket Protocol)
    ↓
Phase 5 (Frontend Stores)
    ↓
Phase 6 (Frontend UI)
    
Phase 7 (Contract Injection) - can be done in parallel with Phase 3-6
```

---

## Key Design Decisions

1. **Todo list stored in database**: Persisted per chat, survives daemon restarts
2. **Injection as user message**: Not persisted to DB, only sent to AI in tool loop
3. **Templates in separate folder**: Easy to modify prompts without code changes
4. **Tool always available in chat**: Not conditional on roles or projects
5. **Frontend button near role selector**: Consistent UI placement
6. **Real-time updates via WebSocket**: Frontend reflects changes immediately
7. **Discarded status `[!]`**: Allows AI to mark items as no longer needed without removing them from history

---

## Template File Structure

```
templates/
├── environment_details_with_role.md
├── environment_details_no_role.md
├── todo_list_empty.md
├── todo_list_with_items.md
└── rhd_set_todo_list_contract.md
```

---

## Future Enhancements (Out of Scope)

- Scenario support for todo lists
- Todo list export/import
- Todo list history/audit log
- Customizable status types
- Todo list persistence across chats
