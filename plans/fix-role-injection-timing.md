# Fix: Role System Prompt Injection Timing

## Problem Statement

Analysis of `output.json` reveals three issues with role system prompt injection:

### Issue 1: Role prompt injected immediately on selection
When user selects a role via UI, `set_active_role()` calls `inject_role_system_prompt()` immediately, adding a system message. This creates orphaned system prompts with no following user message.

**Evidence from output.json (chat 76):**
- Messages 524, 525, 526 are role system prompts injected by rapid role toggling
- No user messages follow them
- These prompts serve no purpose until a user message arrives

### Issue 2: Roles list re-injected on role change
`set_active_role()` calls `reset_roles_list_injected()` which causes the roles list to be re-injected on the next message, even though no new project was attached.

**Evidence from output.json:**
- Message 521 is a roles list re-injection after switching to "horrors"
- No new project was attached, so re-injection is unnecessary

### Issue 3: Role name not quoted
"Current active role is jokes" should be `Current active role is "jokes"` for clarity.

## Expected Behavior

1. Role system prompt should be deferred and injected only before the next user message or tool result
2. Roles list should NOT be re-injected when only the active role changes (only when projects are attached/detached)
3. Role names should be quoted in prompts

## Solution Design

### Database Changes

Add a new column `role_prompt_pending` to the `chats` table to track when a role change needs its system prompt injected.

```sql
ALTER TABLE chats ADD COLUMN role_prompt_pending BOOLEAN NOT NULL DEFAULT 0;
```

### Code Changes

#### 1. `packages/rhd_db/src/chat_db.rs`

Add methods:
- `set_role_prompt_pending(chat_id, pending: bool)` - Set/clear the pending flag
- `has_role_prompt_pending(chat_id) -> bool` - Check if role prompt needs injection

#### 2. `packages/rhd_chat/src/manager.rs`

Modify `set_active_role()`:
- Remove the call to `inject_role_system_prompt()`
- Remove the call to `reset_roles_list_injected()`
- Add call to `set_role_prompt_pending(chat_id, true)`

#### 3. `packages/rhd_chat/src/projects.rs`

Add new function `inject_pending_role_prompt()`:
- Check if `role_prompt_pending` is true
- If true, inject the role system prompt
- Clear the pending flag after injection
- This should be called AFTER `inject_roles_prompt()` to ensure correct ordering

Modify `inject_roles_prompt()`:
- Quote the role name: `Current active role is "{}"` → `Current active role is "{}"`

Modify `inject_role_system_prompt()`:
- Quote the role name in the prompt

#### 4. `packages/rhd_chat/src/stream.rs`

In `send_message()` and `edit_and_resend()`:
- After `inject_roles_prompt()`, add call to `inject_pending_role_prompt()`

#### 5. `packages/rhd_chat/src/tools.rs`

In `tool_loop()`:
- Before each iteration, call `inject_pending_role_prompt()` to handle role changes during tool execution

### Injection Order

When a user message is sent:
1. `inject_system_prompts()` - Project system prompts (if new project attached)
2. `inject_roles_prompt()` - Roles list (if new project with roles attached, or first message)
3. `inject_pending_role_prompt()` - Role system prompt (if role was just changed)
4. User message is added
5. AI processes the messages

### Message Flow After Fix

**Before fix (current behavior):**
```
[role select] → system: "Your current role is now jokes..."
[role select] → system: "Your current role is now horrors..."
[role select] → system: "Your current role is now jokes..."
[user message] → system: roles list (re-injected)
[user message] → user: "Hello"
```

**After fix (expected behavior):**
```
[role select] → (no system message, just set pending flag)
[role select] → (no system message, just update pending flag)
[role select] → (no system message, just update pending flag)
[user message] → system: roles list (only if not already injected)
[user message] → system: "Your current role is now \"jokes\"..."
[user message] → user: "Hello"
```

## Files to Modify

| File | Changes |
|------|---------|
| `packages/rhd_db/src/chat_db.rs` | Add `role_prompt_pending` column and methods |
| `packages/rhd_chat/src/manager.rs` | Modify `set_active_role()` to defer injection |
| `packages/rhd_chat/src/projects.rs` | Add `inject_pending_role_prompt()`, quote role names |
| `packages/rhd_chat/src/stream.rs` | Call `inject_pending_role_prompt()` in send/edit flows |
| `packages/rhd_chat/src/tools.rs` | Call `inject_pending_role_prompt()` in tool loop |

## Testing

### Unit Tests
- Test `set_role_prompt_pending()` and `has_role_prompt_pending()` DB methods
- Test `inject_pending_role_prompt()` with pending flag set/unset
- Test that role name is quoted in prompts

### Integration Tests
- Test role selection via UI followed by user message
- Test rapid role toggling (only last role's prompt should be injected)
- Test role change during tool execution

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Existing chats may have stale `role_prompt_pending` state | Default value is `false`, so existing chats are unaffected |
| Role prompt may be injected at wrong time | Ensure injection happens after roles list injection |
| Tool loop may miss role change | Call `inject_pending_role_prompt()` at start of each iteration |

## Success Criteria

1. Role system prompt is only injected before user message or tool result
2. Roles list is NOT re-injected when only role changes
3. Role names are quoted in all prompts
4. Rapid role toggling results in only one system prompt (for the final role)
5. All existing tests pass
6. New tests cover the fixed behavior
