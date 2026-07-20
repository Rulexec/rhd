# Phase 8: Validation and Coverage Report

## Overview

This phase validates that all test cases are fully covered with step comments and generates a comprehensive coverage report. It verifies that the sum of step ranges in tests covers all test case steps and updates test case files with correct "Covered By" references.

**Scope:**
- Review all test files for step comments
- Verify sum of step ranges covers all test case steps
- Update test case files to reference new tests
- Generate coverage matrix

**Out of scope:**
- Writing new tests (covered in Phases 1-7)
- Adding step comments (covered in Phases 1-2)

## Files to Update

### 1. Test Case Files

Update the "Covered By" section in each test case file to reflect the new test coverage.

#### `tests/cases/chat-abort.md`

**Update "Covered By" section:**

```markdown
## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic, steps 2-7)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering, step 1)
```

#### `tests/cases/chat-delete.md`

**Update "Covered By" section:**

```markdown
## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic, steps 4-8)
- `frontend/src/tests/ui/ChatList.test.ts` (UI rendering, steps 1-3)
```

#### `tests/cases/chat-edit-message.md`

**Update "Covered By" section:**

```markdown
## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic, steps 5-11)
- `frontend/src/tests/ui/Message.test.ts` (UI rendering, steps 1-4)
```

#### `tests/cases/chat-pause-resume.md`

**Update "Covered By" section:**

```markdown
## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic, pause steps 2-7, resume steps 2-7)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering, pause step 1, resume step 1)
```

#### `tests/cases/roles-integration.md`

**Update "Covered By" section:**

```markdown
## Covered By
- `frontend/src/tests/e2e/roles-integration.test.ts` (E2E tests for all 8 scenarios)
- `packages/rhd_chat/src/projects.rs` (backend logic)
- `frontend/src/lib/chatStores.ts` (role stores)
- `frontend/src/lib/stateExport.ts` (state export/import)
- Backend unit tests in `packages/rhd_chat/src/projects.rs`
```

### 2. Coverage Matrix

Generate a coverage matrix showing which test steps are covered by which tests.

#### Coverage Matrix Template

Create a markdown table showing coverage for each test case:

```markdown
# Test Coverage Matrix

## chat-create.md (10 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | User clicks "+ New Chat" button | ChatList.test.ts | UI |
| 2 | Dialog opens with title input field | ChatList.test.ts | UI |
| 3 | User enters chat title | ChatList.test.ts | UI |
| 4 | User clicks "Create" button | ChatList.test.ts | UI |
| 5 | System dispatches `createChat` action | chat-state.test.ts | E2E |
| 6 | System sends request to daemon | chat-state.test.ts | E2E |
| 7 | Daemon creates chat and returns chatId | chat-state.test.ts | E2E |
| 8 | System updates chats store | chat-state.test.ts | E2E |
| 9 | System sets currentChatId | chat-state.test.ts | E2E |
| 10 | Dialog closes | ChatList.test.ts | UI |

**Coverage: 10/10 (100%)**

## chat-send-message.md (14 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | User types message in input field | MessageInput.test.ts | UI |
| 2 | User clicks Send button | MessageInput.test.ts | UI |
| 3 | System dispatches `sendMessage` action | chat-state.test.ts | E2E |
| 4 | System adds user message to messages store | chat-state.test.ts | E2E |
| 5 | System sets isStreaming to true | chat-state.test.ts | E2E |
| 6 | System sends request to daemon | chat-state.test.ts | E2E |
| 7 | Daemon processes message | chat-state.test.ts | E2E |
| 8 | System receives `chatStreamChunk` actions | chat-state.test.ts | E2E |
| 9 | System creates optimistic assistant message | chat-state.test.ts | E2E |
| 10 | System appends chunks to assistant message | chat-state.test.ts | E2E |
| 11 | System receives `chatStreamFinished` action | chat-state.test.ts | E2E |
| 12 | System sets isStreaming to false | chat-state.test.ts | E2E |
| 13 | System receives `chatMessageAdded` | chat-state.test.ts | E2E |
| 14 | System replaces optimistic message | chat-state.test.ts | E2E |

**Coverage: 14/14 (100%)**

## chat-streaming.md (9 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | System receives `chatStreamChunk` action | chat-state.test.ts | E2E |
| 2 | System creates optimistic assistant message | chat-state.test.ts | E2E |
| 3 | System appends content to message | chat-state.test.ts | E2E |
| 4 | System displays animated dots indicator | Message.test.ts | UI |
| 5 | System receives `chatStreamFinished` action | chat-state.test.ts | E2E |
| 6 | System sets isStreaming to false | chat-state.test.ts | E2E |
| 7 | System receives `chatMessageAdded` | chat-state.test.ts | E2E |
| 8 | System replaces optimistic message | chat-state.test.ts | E2E |
| 9 | System clears streamingMessageId | chat-state.test.ts | E2E |

**Coverage: 9/9 (100%)**

## chat-select-model.md (8 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | System loads available models | chat-state.test.ts | E2E |
| 2 | System populates model selector dropdown | MessageInput.test.ts | UI |
| 3 | System auto-selects first model | chat-state.test.ts | E2E |
| 4 | User clicks model selector dropdown | MessageInput.test.ts | UI |
| 5 | User selects a model | MessageInput.test.ts | UI |
| 6 | System dispatches `selectModel` action | chat-state.test.ts | E2E |
| 7 | System updates selectedModel store | chat-state.test.ts | E2E |
| 8 | Model selection persists | chat-state.test.ts | E2E |

**Coverage: 8/8 (100%)**

## chat-mcp-tools.md (19 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1-19 | All steps | chat-mcp-tools.test.ts | E2E |

**Coverage: 19/19 (100%)**

## chat-abort.md (7 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | User clicks Abort button | MessageInput.test.ts | UI |
| 2 | System dispatches `abortChat` action | chat-state.test.ts | E2E |
| 3 | System sends abort request | chat-state.test.ts | E2E |
| 4 | Daemon stops processing | chat-state.test.ts | E2E |
| 5 | System receives stream error/finished | chat-state.test.ts | E2E |
| 6 | System sets isStreaming to false | chat-state.test.ts | E2E |
| 7 | System clears streamingMessageId | chat-state.test.ts | E2E |

**Coverage: 7/7 (100%)**

## chat-delete.md (8 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | User clicks delete button | ChatList.test.ts | UI |
| 2 | System shows confirmation dialog | ChatList.test.ts | UI |
| 3 | User confirms deletion | ChatList.test.ts | UI |
| 4 | System dispatches `deleteChat` action | chat-state.test.ts | E2E |
| 5 | System sends request to daemon | chat-state.test.ts | E2E |
| 6 | Daemon deletes chat | chat-state.test.ts | E2E |
| 7 | System removes chat from chats store | chat-state.test.ts | E2E |
| 8 | System clears currentChatId and messages | chat-state.test.ts | E2E |

**Coverage: 8/8 (100%)**

## chat-edit-message.md (11 steps)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| 1 | User clicks edit button | Message.test.ts | UI |
| 2 | System enters edit mode | Message.test.ts | UI |
| 3 | User modifies message content | Message.test.ts | UI |
| 4 | User clicks Save | Message.test.ts | UI |
| 5 | System dispatches `editMessage` action | chat-state.test.ts | E2E |
| 6 | System updates message content | chat-state.test.ts | E2E |
| 7 | System truncates subsequent messages | chat-state.test.ts | E2E |
| 8 | System sets isStreaming to true | chat-state.test.ts | E2E |
| 9 | System sends request to daemon | chat-state.test.ts | E2E |
| 10 | Daemon re-processes from edited message | chat-state.test.ts | E2E |
| 11 | System receives streaming response | chat-state.test.ts | E2E |

**Coverage: 11/11 (100%)**

## chat-pause-resume.md (14 steps: 7 pause + 7 resume)
| Step | Description | Covered By | Test Type |
|------|-------------|------------|-----------|
| Pause 1 | User clicks Pause button | MessageInput.test.ts | UI |
| Pause 2 | System dispatches `pauseChat` action | chat-state.test.ts | E2E |
| Pause 3 | System sends pause request | chat-state.test.ts | E2E |
| Pause 4 | Daemon pauses execution | chat-state.test.ts | E2E |
| Pause 5 | System receives `chatPaused` action | chat-state.test.ts | E2E |
| Pause 6 | System sets isPaused to true | chat-state.test.ts | E2E |
| Pause 7 | System sets isStreaming to false | chat-state.test.ts | E2E |
| Resume 1 | User clicks Resume button | MessageInput.test.ts | UI |
| Resume 2 | System dispatches `resumeChat` action | chat-state.test.ts | E2E |
| Resume 3 | System sends resume request | chat-state.test.ts | E2E |
| Resume 4 | Daemon resumes execution | chat-state.test.ts | E2E |
| Resume 5 | System receives `chatResumed` action | chat-state.test.ts | E2E |
| Resume 6 | System sets isPaused to false | chat-state.test.ts | E2E |
| Resume 7 | System sets isStreaming to true | chat-state.test.ts | E2E |

**Coverage: 14/14 (100%)**

## roles-integration.md (8 scenarios)
| Scenario | Description | Covered By | Test Type |
|----------|-------------|------------|-----------|
| 1 | Basic Role Selection | roles-integration.test.ts | E2E |
| 2 | Role Switching via Tool | roles-integration.test.ts | E2E |
| 3 | Project Detachment with Active Role | roles-integration.test.ts | E2E |
| 4 | Role Name Conflict | roles-integration.test.ts | E2E |
| 5 | Project Attached After Chat Started | roles-integration.test.ts | E2E |
| 6 | No Roles Available | roles-integration.test.ts | E2E |
| 7 | Multiple Projects with Roles | roles-integration.test.ts | E2E |
| 8 | State Export/Import with Roles | roles-integration.test.ts | E2E |

**Coverage: 8/8 (100%)**

## Summary

| Test Case | Total Steps | Covered | Coverage |
|-----------|-------------|---------|----------|
| chat-create.md | 10 | 10 | 100% |
| chat-send-message.md | 14 | 14 | 100% |
| chat-streaming.md | 9 | 9 | 100% |
| chat-select-model.md | 8 | 8 | 100% |
| chat-mcp-tools.md | 19 | 19 | 100% |
| chat-abort.md | 7 | 7 | 100% |
| chat-delete.md | 8 | 8 | 100% |
| chat-edit-message.md | 11 | 11 | 100% |
| chat-pause-resume.md | 14 | 14 | 100% |
| roles-integration.md | 8 scenarios | 8 | 100% |

**Total Coverage: 100%**
```

## Validation Activities

### 1. Review All Test Files for Step Comments

**Checklist:**
- [ ] `chat-state.test.ts` has step comments for all tests
- [ ] `chat-mcp-tools.test.ts` has step comments for all tests
- [ ] `MessageInput.test.ts` has step comments for all tests
- [ ] `ChatList.test.ts` has step comments for all tests
- [ ] `Message.test.ts` has step comments for all tests
- [ ] `roles-integration.test.ts` has step comments for all tests

### 2. Verify Step Range Coverage

**For each test case:**
- [ ] Sum of step ranges in tests covers all steps
- [ ] No gaps in coverage
- [ ] No duplicate coverage (unless intentional)

### 3. Run All Tests

**Execute:**
```bash
mise run test-frontend-e2e
```

**Verify:**
- [ ] All tests pass
- [ ] No test failures
- [ ] No test timeouts

### 4. Update Test Case Files

**For each test case file:**
- [ ] Update "Covered By" section with correct file references
- [ ] Include step ranges for each test file
- [ ] Verify accuracy of coverage information

### 5. Generate Coverage Report

**Create:**
- [ ] Coverage matrix markdown file
- [ ] Summary table with coverage percentages
- [ ] Detailed step-by-step coverage for each test case

## Implementation Notes

1. **Validation Order**: 
   - First verify all step comments are present
   - Then verify step ranges cover all steps
   - Then run tests to ensure they pass
   - Finally update test case files and generate report

2. **Coverage Calculation**:
   - Count total steps in each test case
   - Count covered steps (from step comments)
   - Calculate percentage: (covered / total) * 100

3. **Gap Analysis**:
   - If any steps are not covered, identify which steps
   - Determine if additional tests are needed
   - Update test cases or add new tests as needed

## Dependencies

- This phase depends on: All previous phases (1-7)
- This phase must be completed after: All other phases
- This is the final validation phase

## Success Criteria

- All test files have step comments
- Sum of step ranges covers all test case steps (100% coverage)
- All tests pass when run with `mise run test-frontend-e2e`
- Test case files updated with correct "Covered By" references
- Coverage matrix shows 100% step coverage for all test cases
- Coverage report generated and saved to `plans/test-coverage-matrix.md`
