# Roles

## Purpose
Roles allow projects to define multiple behavioral personas that can be switched during a chat. Each role has its own system prompt and "when to use" description. Users can select roles via the UI, or the AI can switch roles automatically using the `rhd_set_role` tool.

## What Is a Role?
A role is a sub-directory within a project's `roles/` folder containing:
- `systemPrompt.md` — The role's system prompt (required)
- `whenToUse.md` — Description for when to use this role (required)

Role name is derived from the directory name.

## Project Structure

```
projects/
└── my-project/
    ├── mcp.yaml
    ├── systemPrompt.md
    └── roles/
        ├── developer/
        │   ├── systemPrompt.md
        │   └── whenToUse.md
        └── reviewer/
            ├── systemPrompt.md
            └── whenToUse.md
```

## How It Works

### Role Selection
1. User attaches a project with roles to a chat
2. System emits `RolesUpdated` event with available roles
3. WebSocket client receives available roles via event
4. User selects a role from the dropdown
5. System injects the role's system prompt as a system message
6. AI behavior changes according to the selected role

### AI-Driven Role Switching
- AI can call `rhd_set_role` tool to switch roles automatically
- Tool is only available when roles are defined in attached projects
- When AI switches roles, the role's system prompt is injected immediately
- WebSocket client receives `RoleChanged` event

### Role Injection
- Roles list prompt is injected on the first message after project attachment
- Role system prompt is injected when a role is selected
- Role names are quoted in prompts for clarity (e.g., `Current active role is "developer"`)
- Roles list includes all roles from all attached projects

### Conflict Detection
- When attaching a project, the system checks for duplicate role names across all attached projects
- If duplicate role names are found, attachment is rejected with an error
- Error lists the conflicting role names
- User must detach one of the conflicting projects or rename roles

## WebSocket Protocol

### Requests
- `setRole` — Set active role (chatId, projectName, roleName)
- `getAvailableRoles` — Get all available roles for a chat (chatId)
- `clearActiveRole` — Clear active role (chatId)

### Response Format
```json
{
  "roles": [
    {
      "projectName": "my-project",
      "roleName": "developer",
      "whenToUse": "Use for coding tasks"
    }
  ],
  "activeRoleProject": "my-project",
  "activeRoleName": "developer"
}
```

## Error Handling

### Role Not Found
- If AI calls `rhd_set_role` with a non-existent role name, tool returns error
- Error message: `Error: role 'roleName' not found in any attached project`

### Role Name Conflict
- Attachment rejected with list of conflicting role names
- Error message: `Role names already in use: developer, reviewer`
- User must resolve conflict before attaching

### Project Detachment
- When a project is detached, if its role was active, the active role is cleared
- `ActiveRoleCleared` event is emitted
- Roles list is updated via `RolesUpdated` event

## Edge Cases

### Project Attached After Chat Started
- Roles become available immediately
- `RolesUpdated` event is emitted
- Roles become available to WebSocket clients
- Roles list prompt is injected on next message

### No Roles Available
- No roles are available to WebSocket clients
- `rhd_set_role` tool is not available
- Roles list prompt is not injected

### Role Switching During Streaming
- Role switching via WebSocket is disabled during streaming
- AI can still switch roles via `rhd_set_role` tool during tool loop
- Role change takes effect immediately for subsequent AI calls
