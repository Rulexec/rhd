# Test Cases

## Overview

Test cases are markdown files in `tests/cases/` that describe user scenarios step-by-step. They serve as the single source of truth for what needs to be tested and are cross-linked to actual test implementations.

## Test Case Format

Each test case file follows this structure:

```markdown
# Test Case: <Name>

## Description
Brief description of the user scenario.

## Preconditions
- List of conditions that must be true before the test starts
- e.g., "Chat exists and is selected", "WebSocket connected"

## Steps
1. User types message in input field
2. User clicks Send button
3. System dispatches `sendMessage` action
...

## Expected Results
- List of expected outcomes
- e.g., "User message appears in message list"

## Actions
- `actionName` — description of when this action is dispatched/received
- List all actions involved in the test case

## Covered By

### E2E Tests
- [`test-file.test.ts`](path/to/test) - `test name` (steps X-Y)

### UI Tests
- [`test-file.test.ts`](path/to/test) - `test name` (step Z)

```

## Step Comments in Test Code

Tests must include comments that reference the test case steps they cover. This makes it easy to trace which test code covers which test case step.

### Format

```typescript
// Covers test-case-name.md steps X-Y
test('test description', async () => {
  // Step 1. User does something
  await performAction();
  
  // Steps 2-3. System processes and responds
  await expect(result).toBeVisible();
  
  // Step 4. User verifies outcome
  await expect(outcome).toBeTruthy();
});
```

### Rules

1. **First line comment**: Each test file/test should start with a comment indicating which test case(s) it covers and which steps
2. **Step comments**: Inline comments before code blocks indicate which step(s) that code covers
3. **Step ranges**: Tests can cover ranges of steps (e.g., "Steps 2-3")
4. **Complete coverage**: The sum of all step ranges in tests must cover all steps in the test case
5. **Cross-linking**: Test case files must list all tests that cover them in the "Covered By" section

### Example

```typescript
// Covers chat-send-message.md steps 3-14
test('sends message and receives streaming response', async () => {
  // Step 3. System dispatches sendMessage action
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello' } });
  
  // Steps 4-5. System adds user message and sets streaming state
  const messages = get(messagesStore);
  expect(messages.length).toBe(1);
  expect(get(isStreaming)).toBe(true);
  
  // Steps 6-7. System sends request and daemon processes
  // (handled by mock server in E2E tests)
  
  // Steps 8-10. System receives streaming chunks
  await dispatch({ type: 'chatStreamChunk', payload: { content: 'Hi' } });
  await expect(screen.getByText('Hi')).toBeVisible();
  
  // Steps 11-14. System finishes stream and updates state
  await dispatch({ type: 'chatStreamFinished' });
  expect(get(isStreaming)).toBe(false);
});
```

## Cross-Linking Guidelines

1. **Test case → Test file**: List all tests that cover this case in "Covered By" section
2. **Test file → Test case**: First line comment indicates which test case is covered
3. **Step mapping**: Comments in test code indicate which steps are covered
4. **Coverage verification**: Sum of step ranges must equal all steps in test case

## Creating New Test Cases

1. Create markdown file in `tests/cases/` with descriptive name
2. Follow the format above
3. List all steps from user perspective
4. Implement tests with step comments
5. Update "Covered By" section with test references
6. Verify complete step coverage

