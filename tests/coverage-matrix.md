# Test Coverage Matrix

This document provides a comprehensive overview of test coverage for all test cases.

## Summary Table

| Test Case | Total Steps | E2E Coverage | UI Coverage | Total Coverage |
|-----------|-------------|--------------|-------------|----------------|
| [chat-create.md](cases/chat-create.md) | 10 | Steps 5-9 | Steps 1, 8-9 | 60% |
| [chat-send-message.md](cases/chat-send-message.md) | 14 | Steps 3-14 | Step 2, preconditions | 93% |
| [chat-streaming.md](cases/chat-streaming.md) | 9 | Steps 1-9 | Step 4, 8 | 100% |
| [chat-select-model.md](cases/chat-select-model.md) | 8 | Steps 1-3, 6-8 | Step 2 | 75% |
| [chat-mcp-tools.md](cases/chat-mcp-tools.md) | 19 | Steps 1-19 | N/A | 100% |
| [chat-abort.md](cases/chat-abort.md) | 7 | Steps 2-3 | Step 1 | 43% |
| [chat-delete.md](cases/chat-delete.md) | 8 | Steps 4-8 | Step 1 | 75% |
| [chat-edit-message.md](cases/chat-edit-message.md) | 11 | Steps 5-11 | Step 1 | 73% |
| [chat-pause-resume.md](cases/chat-pause-resume.md) | 14 (7+7) | **SKIPPED** | Pause step 1, Resume step 1 | 14% |
| [roles-integration.md](cases/roles-integration.md) | 8 scenarios | All 8 scenarios | N/A | 100% |

## Detailed Coverage by Test Case

### chat-create.md (60% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `creates chat via daemon and updates state` (steps 5-9)
- **UI Tests:**
  - [`ChatList.test.ts`](../frontend/src/tests/ui/ChatList.test.ts) - `renders new chat button` (step 1)
  - [`ChatList.test.ts`](../frontend/src/tests/ui/ChatList.test.ts) - `renders chat list items` (step 8)
  - [`ChatList.test.ts`](../frontend/src/tests/ui/ChatList.test.ts) - `highlights selected chat` (step 9)
- **Not Covered:** Steps 2-4, 10 (dialog UI interactions)

### chat-send-message.md (93% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `sends message and receives streaming response from daemon` (steps 3-14)
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `handles multiple messages in sequence` (steps 3-14, multiple iterations)
- **UI Tests:**
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders send button when not streaming` (step 2)
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `disables send button when no chat selected` (preconditions)
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `disables send button when no model selected` (preconditions)
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `shows error message when streamError is set` (steps 12-13 area)
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders model selector with available models` (preconditions)
- **Not Covered:** Step 1 (user types message)

### chat-streaming.md (100% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `sends message and receives streaming response from daemon` (steps 1-9, state logic)
- **UI Tests:**
  - [`Message.test.ts`](../frontend/src/tests/ui/Message.test.ts) - `shows streaming dots for streaming message` (step 4)
  - [`Message.test.ts`](../frontend/src/tests/ui/Message.test.ts) - `renders assistant message content` (step 8)
  - [`Message.test.ts`](../frontend/src/tests/ui/Message.test.ts) - `renders thinking content when present` (step 8)
  - [`Message.test.ts`](../frontend/src/tests/ui/Message.test.ts) - `renders system message with collapsible header` (step 8)

### chat-select-model.md (75% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `loads available models from daemon` (step 1)
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `auto-selects first model when available` (steps 1-3, 6-8)
- **UI Tests:**
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders model selector with available models` (step 2)
- **Not Covered:** Steps 4-5 (user interaction with dropdown)

### chat-mcp-tools.md (100% coverage)
- **E2E Tests:**
  - [`chat-mcp-tools.test.ts`](../frontend/src/tests/e2e/chat-mcp-tools.test.ts) - `attaches project with MCP and streams final response after tool call` (steps 1-19)
  - [`chat-mcp-tools.test.ts`](../frontend/src/tests/e2e/chat-mcp-tools.test.ts) - `handles multiple tool calls with different results` (steps 1-19, single iteration)
  - [`chat-mcp-tools.test.ts`](../frontend/src/tests/e2e/chat-mcp-tools.test.ts) - `maintains unique tool call ids across multiple iterations` (steps 1-19, two iterations)

### chat-abort.md (43% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `aborts streaming chat and updates state` (steps 2-3)
- **UI Tests:**
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1)
- **Not Covered:** Steps 4-7 (require daemon to send `chatStreamError` or `chatStreamFinished` event)

### chat-delete.md (75% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `deletes chat and updates state` (steps 4-8)
- **UI Tests:**
  - [`ChatList.test.ts`](../frontend/src/tests/ui/ChatList.test.ts) - `renders delete button for each chat` (step 1)
- **Not Covered:** Steps 2-3 (confirmation dialog)

### chat-edit-message.md (73% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `edits message and re-streams response` (steps 5-11)
- **UI Tests:**
  - [`Message.test.ts`](../frontend/src/tests/ui/Message.test.ts) - `renders edit button for user messages` (step 1)
- **Not Covered:** Steps 2-4 (edit mode UI)

### chat-pause-resume.md (14% coverage)
- **E2E Tests:**
  - [`chat-state.test.ts`](../frontend/src/tests/e2e/chat-state.test.ts) - `pauses and resumes streaming chat` (pause steps 2-7, resume steps 2-7) - **SKIPPED**
- **UI Tests:**
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (pause step 1)
  - [`MessageInput.test.ts`](../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (resume step 1)
- **Not Covered:** Pause steps 2-7, Resume steps 2-7 (E2E test is skipped)

### roles-integration.md (100% coverage)
- **E2E Tests:**
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 1: Basic Role Selection` (steps 1-8)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 2: Role Switching via Tool` (steps 1-6)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 3: Project Detachment with Active Role` (steps 1-7)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 4: Role Name Conflict` (steps 1-6)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 5: Project Attached After Chat Started` (steps 1-8)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 6: No Roles Available` (steps 1-6)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 7: Multiple Projects with Roles` (steps 1-6)
  - [`roles-integration.test.ts`](../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 8: State Export/Import with Roles` (steps 1-7)

## Coverage Gaps

The following test cases have incomplete coverage and may need additional tests:

1. **chat-abort.md** (43%) - Missing coverage for steps 4-7 (daemon response handling)
2. **chat-pause-resume.md** (14%) - E2E test is skipped, missing coverage for pause/resume state logic
3. **chat-create.md** (60%) - Missing coverage for dialog UI interactions (steps 2-4, 10)
4. **chat-delete.md** (75%) - Missing coverage for confirmation dialog (steps 2-3)
5. **chat-edit-message.md** (73%) - Missing coverage for edit mode UI (steps 2-4)
6. **chat-select-model.md** (75%) - Missing coverage for user dropdown interaction (steps 4-5)

## Recommendations

1. **Enable chat-pause-resume E2E test** - Remove `it.skip` and ensure the test passes
2. **Add dialog component tests** - Create tests for dialog UI interactions in chat-create and chat-delete
3. **Add edit mode UI tests** - Create tests for the edit mode interface in chat-edit-message
4. **Mock daemon responses** - For chat-abort, mock the daemon's `chatStreamError` response to test steps 4-7
