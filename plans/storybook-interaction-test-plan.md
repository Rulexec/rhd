# Storybook Interaction Test Plan

**Status: NOT IMPLEMENTED**

This plan was not implemented. The approach using `@storybook/test-runner` was abandoned in favor of direct Playwright testing.

**Implemented by:** [storybook-playwright-direct.md](./storybook-playwright-direct.md)

---

# Storybook Interaction Test Plan

## Goal

Implement a Storybook test-runner test for the `Pause-Abort/ChatView` story that:
1. Clicks on the step action buttons in sequence
2. Verifies the intermediate and final states are correct
3. Ensures the story execution flow works as expected

## Current State

- Story location: `frontend/src/stories/pause-abort/ChatView.stories.ts`
- Story has 2 steps:
  - Step 1: "Add user message" - adds user message and shows loading indicator
  - Step 2: "Receive AI response" - emits stream chunks and finishes streaming
- Storybook is configured with `@storybook/test` and `@storybook/addon-interactions`
- No existing Storybook interaction tests in the project

## Approach

Use `@storybook/test-runner` to:
1. Run tests against the Storybook instance
2. Use Playwright to interact with the story
3. Click the step buttons and verify state changes
4. Assert final state (messages, streaming indicators, etc.)

## Implementation Steps

### Step 1: Install @storybook/test-runner

Add `@storybook/test-runner` as a dev dependency:
```bash
npm install --save-dev @storybook/test-runner
```

### Step 2: Configure test-runner

Create `frontend/.storybook/test-runner.ts` with configuration:
```typescript
import type { TestRunnerConfig } from '@storybook/test-runner';

const config: TestRunnerConfig = {
  setup() {
    // Setup code if needed
  },
};

export default config;
```

### Step 3: Add test IDs to testIds.ts

Add test ID helpers to `frontend/src/stories/testIds.ts`:
```typescript
export const STEP_BUTTON_PREFIX = 'step-button';

export function stepButtonTestId(stepIndex: number): string {
  return `${STEP_BUTTON_PREFIX}-${stepIndex}`;
}
```

### Step 4: Add test IDs to StoryExecutionControls

Update `frontend/src/stories/StoryExecutionControls.svelte` to use the test ID helper:
```svelte
<script lang="ts">
  import { stepButtonTestId } from '@/stories/testIds';
  // ... existing code
</script>

<button
  onclick={executeStep}
  disabled={isExecuting || isComplete}
  class="execute-btn"
  data-testid={stepButtonTestId(currentStepIndex)}
>
```

### Step 5: Create test file

Create `frontend/src/stories/pause-abort/ChatView.stories.test.ts`:
```typescript
import { test, expect } from '@storybook/test-runner';
import { getPlaywrightBrowser } from '@storybook/test-runner/playwright';

test('ChatView WithControls - step execution flow', async ({ page }) => {
  // Navigate to the story
  await page.goto('/iframe.html?id=pause-abort-chatview--with-controls&viewMode=story');
  
  // Wait for story to load
  await page.waitForSelector('[data-testid="step-button-0"]');
  
  // Step 1: Click "Add user message" button
  const step1Button = page.locator('[data-testid="step-button-0"]');
  await step1Button.click();
  
  // Wait for user message to appear
  await expect(page.getByText('Hello, how are you?')).toBeVisible();
  
  // Verify loading indicator is shown (Pause/Abort buttons)
  await expect(page.getByText('Pause')).toBeVisible();
  await expect(page.getByText('Abort')).toBeVisible();
  
  // Step 2: Click "Receive AI response" button
  const step2Button = page.locator('[data-testid="step-button-1"]');
  await step2Button.click();
  
  // Wait for AI message to appear
  await expect(page.getByText(/I'm doing well, thank you!/)).toBeVisible();
  
  // Verify streaming dots are gone
  const streamingDots = page.locator('.streaming-dots');
  await expect(streamingDots).toHaveCount(0);
  
  // Verify Send button is re-enabled
  const sendButton = page.locator('[data-testid="send-button"]');
  await expect(sendButton).toBeEnabled();
});
```

### Step 6: Add test script to package.json

Add test script to `frontend/package.json`:
```json
{
  "scripts": {
    "test-storybook": "test-storybook"
  }
}
```

## File Changes

### 1. `frontend/src/stories/testIds.ts`

Add test ID helpers:
```typescript
export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
} as const;

export const STEP_BUTTON_PREFIX = 'step-button';

export function stepButtonTestId(stepIndex: number): string {
  return `${STEP_BUTTON_PREFIX}-${stepIndex}`;
}
```

### 2. `frontend/src/stories/StoryExecutionControls.svelte`

Import and use the test ID helper:
```svelte
<script lang="ts">
  import { stepButtonTestId } from '@/stories/testIds';
  // ... existing imports and code
</script>

<button
  onclick={executeStep}
  disabled={isExecuting || isComplete}
  class="execute-btn"
  data-testid={stepButtonTestId(currentStepIndex)}
>
```

### 3. `frontend/src/stories/pause-abort/ChatView.stories.test.ts` (new file)

Create the test file with Playwright-based tests.

### 4. `frontend/.storybook/test-runner.ts` (new file)

Create test-runner configuration.

### 5. `frontend/package.json`

Add `@storybook/test-runner` dependency and test script.

## Risks

1. **Timing issues**: The story uses `sleep()` for async operations. The test needs to wait for these operations to complete. Use Playwright's auto-waiting and explicit waits.

2. **State isolation**: Each story render should have isolated state. The `setupDefaultStores()` is called in the decorator, which should reset state between runs.

3. **Mock WS handler**: The mock WS handler is set up in step 1's execute function. The test needs to ensure the handler is in place before clicking the send button.

4. **Streaming dots visibility**: The streaming dots are shown when `streamingMessageId` matches the message ID. The test needs to verify they're removed after `chatStreamFinished` is emitted.

5. **Storybook server**: The test-runner requires a running Storybook server. Tests should be run with `npm run storybook` in one terminal and `npm run test-storybook` in another, or use CI configuration.

## Success Criteria

- [ ] `@storybook/test-runner` is installed and configured
- [ ] Test file is created with Playwright-based tests
- [ ] Test clicks step 1 button and verifies user message appears
- [ ] Test verifies loading indicator (Pause/Abort buttons) appears after step 1
- [ ] Test clicks step 2 button and verifies AI message appears
- [ ] Test verifies streaming dots are removed after step 2
- [ ] Test verifies Send button is re-enabled after step 2
- [ ] Test passes consistently without flakiness
- [ ] Test can be run with `npm run test-storybook`

## Testing the Test

After implementation:
1. Start Storybook: `npm run storybook`
2. In another terminal, run tests: `npm run test-storybook`
3. Verify all tests pass

## Future Enhancements

- Add more stories with different scenarios (pause, abort, error cases)
- Add visual regression tests using Storybook's visual testing addon
- Add accessibility tests using Storybook's accessibility addon
- Configure CI to run Storybook tests automatically
