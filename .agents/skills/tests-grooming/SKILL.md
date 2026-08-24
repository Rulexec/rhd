---
name: tests-grooming
description: Investigates test cases and actual test code, then updates them to match the established pattern with step-by-step comments and coverage references. Use to keep tests/cases/ and frontend/src/tests/ consistent and traceable.
disable-model-invocation: true
---

# Tests Grooming Skill

## Overview

This skill investigates current test cases and actual test code, then updates them to match the established pattern with step-by-step comments and coverage references. It ensures consistency across all test files and maintains traceability between test cases (in `tests/cases/`) and test implementations (in `frontend/src/tests/`).

## Input Requirements

You will need access to:
1. **Test Cases Directory**: `tests/cases/` - Contains markdown files defining test scenarios with numbered steps
2. **Test Code Directory**: `frontend/src/tests/` - Contains actual test implementations (E2E and UI tests)
3. **Coverage Matrix**: `tests/coverage-matrix.md` (if exists) - Shows current coverage status

## Process

### 1. Understand the Established Pattern

**Step Comment Format:**
```typescript
// Covers test-case.md steps X-Y (state logic)
// Steps A-B are UI tests (see TestFile.test.ts)

// Step N. Description matching test case step
await dispatch({ type: 'action', payload: { ... } });

// Step N+1. Next step description
expect(get(store)).toBe(expectedValue);
```

**Key Pattern Elements:**
1. **Range Comment**: At test start, indicate which steps are covered: `// Covers test-case.md steps X-Y`
2. **Step Comments**: Before each action/assertion, add `// Step N. Description`
3. **UI vs State Logic**: Clearly indicate which steps are covered by state logic tests vs UI tests
4. **Setup Steps**: Mark setup steps clearly (e.g., "Setup: Create chat first")
5. **Cross-References**: Reference other test files when steps are covered elsewhere

**"Covered By" Section Format:**
```markdown
## Covered By

### E2E Tests
- [`test-file.test.ts`](frontend/src/tests/e2e/test-file.test.ts) - `test name` (steps X-Y)

### UI Tests
- [`test-file.test.ts`](frontend/src/tests/ui/test-file.test.ts) - `test name` (steps A-B)
```

### 2. Investigate Test Cases

**Read all test case files:**
```bash
ls tests/cases/
cat tests/cases/*.md
```

**For each test case, identify:**
- Total number of steps
- Step descriptions
- Which steps are UI-related vs state logic
- Current "Covered By" section (if exists)

**Create a mapping:**
```
Test Case: chat-example.md
Total Steps: 10
Steps 1-3: UI interactions
Steps 4-8: State logic
Steps 9-10: UI interactions
```

### 3. Investigate Test Code

**Read all test files:**
```bash
# E2E tests
ls frontend/src/tests/e2e/
cat frontend/src/tests/e2e/*.test.ts

# UI tests
ls frontend/src/tests/ui/
cat frontend/src/tests/ui/*.test.ts
```

**For each test file, identify:**
- Which test cases it covers
- Which steps are covered by each test
- Whether step comments are present
- Whether comments match the established pattern

### 4. Identify Deviations

**Check for missing step comments:**
- Tests without `// Covers test-case.md steps X-Y` at the start
- Tests without `// Step N. Description` before actions/assertions
- Tests with incomplete step coverage

**Check for incorrect step comments:**
- Step numbers that don't match test case steps
- Step descriptions that don't match test case descriptions
- Incorrect range coverage (e.g., claims to cover steps 1-10 but only covers 1-5)

**Check for missing "Covered By" sections:**
- Test case files without "Covered By" sections
- "Covered By" sections that don't reference all tests covering the case
- Incorrect step ranges in "Covered By" sections

**Check for test coverage gaps:**
- Test cases with no corresponding tests
- Steps that are not covered by any test
- Tests that don't verify all expected behaviors

### 5. Update Tests to Match Pattern

**Add missing step comments:**
```typescript
it('test description', async () => {
  // Covers test-case.md steps X-Y (state logic)
  // Steps A-B are UI tests (see TestFile.test.ts)
  
  // Step X. Description from test case
  await dispatch({ type: 'action', payload: { ... } });
  
  // Step X+1. Next description
  expect(get(store)).toBe(expectedValue);
});
```

**Update existing step comments:**
- Ensure step numbers match test case steps
- Ensure descriptions match test case descriptions
- Ensure range coverage is accurate

**Update "Covered By" sections:**
```markdown
## Covered By

### E2E Tests
- [`test-file.test.ts`](frontend/src/tests/e2e/test-file.test.ts) - `test name` (steps X-Y)

### UI Tests
- [`test-file.test.ts`](frontend/src/tests/ui/test-file.test.ts) - `test name` (steps A-B)
```

**Add missing tests:**
If a test case has steps not covered by any test:
1. Determine if it should be an E2E test or UI test
2. Create the test following the established pattern
3. Add step comments for all covered steps
4. Update the test case's "Covered By" section

### 6. Validate Implementation

**Run checks:**
```bash
mise run check-svelte
mise run test-frontend-unit
mise run test-frontend-e2e
```

**Verify:**
- All tests pass
- Step comments are present and accurate
- "Covered By" sections are complete
- Coverage matrix is up to date

**Update coverage matrix:**
If `tests/coverage-matrix.md` exists, update it with new coverage information.

### 7. Commit Changes

**Stage all changes:**
```bash
git add -A
```

**Create commit message:**
- Format: lowercase, no period, concise summary
- Examples:
  - "add step comments to new test cases"
  - "update test coverage references"
  - "fix step comments in E2E tests"

**Commit:**
```bash
git commit -m "<commit message>"
```

## Best Practices

### Step Comments

**Be Precise:**
- Match step numbers exactly to test case steps
- Match descriptions closely to test case descriptions
- Use the exact format: `// Step N. Description`

**Indicate Coverage Clearly:**
- Use range comments at test start: `// Covers test-case.md steps X-Y`
- Indicate which steps are UI vs state logic
- Reference other test files when appropriate

**Handle Setup Steps:**
- Mark setup steps clearly: `// Setup: Create chat first`
- Don't count setup steps as covered steps unless they test specific behavior

### "Covered By" Sections

**Be Complete:**
- List all tests that cover the test case
- Include both E2E and UI tests
- Specify exact step ranges for each test

**Use Correct Format:**
- Use markdown links to test files
- Include test name in backticks
- Specify step ranges in parentheses

### Test Coverage

**Aim for 100%:**
- Every step should be covered by at least one test
- Steps can be covered by multiple tests (UI + E2E)
- Document any gaps in coverage matrix

**Test the Right Things:**
- E2E tests for state logic
- UI tests for rendering and interactions
- Don't duplicate coverage unnecessarily

## Examples

### Example 1: Adding Step Comments to Existing Test

**Before:**
```typescript
it('creates chat', async () => {
  await dispatch({ type: 'createChat', payload: { title: 'Test' } });
  const allChats = get(chats);
  expect(allChats.length).toBe(1);
});
```

**After:**
```typescript
it('creates chat', async () => {
  // Covers chat-create.md steps 5-9 (state logic)
  // Steps 1-4, 10 are UI tests (see ChatList.test.ts)
  
  // Step 5. System dispatches `createChat` action with title
  // Step 6. System sends request to daemon
  await dispatch({ type: 'createChat', payload: { title: 'Test' } });
  
  // Step 7. Daemon creates chat and returns chatId
  // Step 8. System updates chats store with new chat
  const allChats = get(chats);
  expect(allChats.length).toBe(1);
  
  // Step 9. System sets currentChatId to new chat
  expect(get(currentChatId)).toBeDefined();
});
```

### Example 2: Updating "Covered By" Section

**Before:**
```markdown
## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts)
```

**After:**
```markdown
## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `creates chat via daemon and updates state` (steps 5-9)

### UI Tests
- [`ChatList.test.ts`](frontend/src/tests/ui/ChatList.test.ts) - `creates chat via UI` (steps 1-4, 10)
```

### Example 3: Adding Missing Test

**Test Case (chat-abort.md):**
```markdown
## Steps
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon stops processing
5. System receives `chatStreamError` or `chatStreamFinished` action
6. System sets isStreaming to false
7. System clears streamingMessageId
```

**Added Test:**
```typescript
it('aborts streaming chat and updates state', async () => {
  // Covers chat-abort.md steps 2-3 (state logic)
  // Step 1 is UI test (see MessageInput.test.ts)
  // Steps 4-7 require daemon to send events (not simulated by mock)
  
  // Setup: Create chat and start streaming
  await dispatch({ type: 'createChat', payload: { title: 'Test' } });
  await configureMock('Streaming response');
  const model = get(availableModels)[0] || 'test_model';
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });
  
  await waitFor(() => {
    expect(get(isStreaming)).toBe(true);
  }, { timeout: 5000 });
  
  // Step 2. System dispatches `abortChat` action
  // Step 3. System sends abort request to daemon
  await dispatch({ type: 'abortChat' });
  
  // Note: Steps 4-7 require daemon-side implementation
});
```

## Checklist

Before committing, verify:

- [ ] All test cases have been investigated
- [ ] All test files have been investigated
- [ ] Step comments are present in all tests
- [ ] Step comments match test case steps
- [ ] Range coverage is accurate
- [ ] "Covered By" sections are complete
- [ ] Coverage matrix is up to date
- [ ] All tests pass
- [ ] No test logic was modified (only comments added)
- [ ] Commit message follows conventions

## Conclusion

This skill provides a systematic approach to maintaining consistency between test cases and test implementations. By following the established pattern with step comments and coverage references, you ensure that tests are traceable, maintainable, and provide clear coverage information.
