# Storybook Playwright Direct Testing Plan

## Overview

Replace the current Vitest-based Storybook testing approach with direct Playwright testing against a running Storybook instance. This provides more reliable browser-based testing without the complexity of Storybook addon integrations.

## Current State (Files to Remove)

The following files were created during the Vitest migration and should be removed:

### Configuration Files
- `frontend/vitest.config.storybook.ts` - Vitest configuration for Storybook tests
- `frontend/src/stories/vitest.setup.ts` - Vitest setup file with jest-dom imports

### Test Files
- `frontend/src/stories/pause-abort/ChatView.stories.test.ts` - Current Vitest-based test

### Modified Files (Need Reversion)
- `frontend/package.json` - Remove vitest-storybook scripts and @storybook/addon-vitest dependency (keep vitest for unit/e2e tests)
- `frontend/.storybook/main.ts` - Remove @storybook/addon-vitest from addons
- `frontend/src/stories/pause-abort/ChatView.stories.ts` - Revert export of storyDefinition
- `frontend/src/stories/StoryExecutionControls.svelte` - Revert data-testid additions
- `frontend/src/stories/testIds.ts` - Revert stepButtonTestId additions

## New Approach: Direct Playwright Testing

### Architecture

```
┌─────────────────┐         ┌──────────────────┐
│   Storybook     │◄────────│   Playwright     │
│   (port 6006)   │         │   Test Runner    │
└─────────────────┘         └──────────────────┘
        │                            │
        │    HTTP/WebSocket          │
        └────────────────────────────┘
```

### Benefits
1. **Simpler Setup** - No complex addon integrations
2. **Real Browser Testing** - Tests run in actual browser environment
3. **Better Debugging** - Can use Playwright's debugging tools
4. **More Reliable** - Direct browser automation without abstraction layers
5. **Flexible** - Can test any Storybook story without special configuration

## Implementation Steps

### Step 1: Clean Up Previous Migration

**Remove Files:**
```bash
rm frontend/vitest.config.storybook.ts
rm frontend/src/stories/vitest.setup.ts
rm frontend/src/stories/pause-abort/ChatView.stories.test.ts
```

**Revert Modified Files:**
- `frontend/package.json` - Remove test-storybook script, remove @storybook/addon-vitest
- `frontend/.storybook/main.ts` - Remove @storybook/addon-vitest from addons
- `frontend/src/stories/pause-abort/ChatView.stories.ts` - Remove export from storyDefinition
- `frontend/src/stories/StoryExecutionControls.svelte` - Remove data-testid additions
- `frontend/src/stories/testIds.ts` - Remove stepButtonTestId function

### Step 2: Install Playwright

Add Playwright to devDependencies:
```json
{
  "devDependencies": {
    "@playwright/test": "^1.42.0"
  }
}
```

### Step 3: Create Playwright Configuration

**Create:** `frontend/playwright.config.ts`

```typescript
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/storybook',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:6006',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npm run storybook',
    url: 'http://localhost:6006',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
```

### Step 4: Create Test Directory Structure

**Create:** `frontend/tests/storybook/`

### Step 5: Migrate Test to Playwright

**Create:** `frontend/tests/storybook/chatview.spec.ts`

```typescript
import { test, expect } from '@playwright/test';

test.describe('ChatView Storybook Tests', () => {
  test('step execution flow', async ({ page }) => {
    // Navigate to the story
    await page.goto('/iframe.html?id=pause-abort-chatview--with-controls&viewMode=story');
    
    // Wait for step button to appear
    const stepButton0 = page.locator('[data-testid="step-button-0"]');
    await expect(stepButton0).toBeVisible();
    
    // Click first step
    await stepButton0.click();
    
    // Wait for user message to appear
    await expect(page.locator('text=Hello, how are you?')).toBeVisible();
    
    // Verify Pause and Abort buttons are visible
    await expect(page.locator('text=Pause')).toBeVisible();
    await expect(page.locator('text=Abort')).toBeVisible();
    
    // Click second step
    const stepButton1 = page.locator('[data-testid="step-button-1"]');
    await stepButton1.click();
    
    // Wait for AI response
    await expect(page.locator('text=I\'m doing well, thank you!')).toBeVisible();
    
    // Verify no streaming dots
    const streamingDots = page.locator('.streaming-dots');
    await expect(streamingDots).toHaveCount(0);
    
    // Verify send button is enabled
    const sendButton = page.locator('[data-testid="send-button"]');
    await expect(sendButton).toBeEnabled();
  });
});
```

### Step 6: Update package.json Scripts

**Add:**
```json
{
  "scripts": {
    "test-storybook": "playwright test",
    "test-storybook:ui": "playwright test --ui",
    "test-storybook:debug": "playwright test --debug"
  }
}
```

### Step 7: Add data-testid Attributes (If Needed)

All test IDs should be defined in `frontend/src/stories/testIds.ts`. If the step buttons don't have data-testid attributes, we need to:

1. Add the test ID helper to `testIds.ts`:
```typescript
export const STEP_BUTTON_PREFIX = 'step-button';

export function stepButtonTestId(stepIndex: number): string {
  return `${STEP_BUTTON_PREFIX}-${stepIndex}`;
}
```

2. Update `StoryExecutionControls.svelte` to use it:
```svelte
<script lang="ts">
  import { stepButtonTestId } from '@/stories/testIds';
  // ... rest of imports
</script>

<button
  onclick={executeStep}
  disabled={isExecuting || isComplete}
  class="execute-btn"
  data-testid={stepButtonTestId(currentStepIndex)}
>
  {#if isExecuting}
    Executing...
  {:else if isComplete}
    Complete
  {:else}
    Execute: {step.name}
  {/if}
</button>
```

## File Changes Summary

| File | Action | Description |
|------|--------|-------------|
| `frontend/vitest.config.storybook.ts` | **DELETE** | No longer needed |
| `frontend/src/stories/vitest.setup.ts` | **DELETE** | No longer needed |
| `frontend/src/stories/pause-abort/ChatView.stories.test.ts` | **DELETE** | Replaced by Playwright test |
| `frontend/package.json` | Modify | Remove vitest deps, add playwright, update scripts |
| `frontend/.storybook/main.ts` | Modify | Remove @storybook/addon-vitest |
| `frontend/src/stories/pause-abort/ChatView.stories.ts` | Modify | Revert export of storyDefinition |
| `frontend/src/stories/StoryExecutionControls.svelte` | Modify | Add data-testid to step buttons |
| `frontend/src/stories/testIds.ts` | Modify | Revert stepButtonTestId additions |
| `frontend/playwright.config.ts` | **CREATE** | Playwright configuration |
| `frontend/tests/storybook/chatview.spec.ts` | **CREATE** | Playwright test file |

## Testing Workflow

### Local Development
```bash
# Run tests (automatically starts Storybook)
npm run test-storybook

# Run tests with UI mode
npm run test-storybook:ui

# Debug tests
npm run test-storybook:debug
```

### CI/CD
```bash
# Install Playwright browsers
npx playwright install --with-deps

# Run tests
npm run test-storybook
```

## Advantages of This Approach

1. **Simplicity** - Direct browser testing without complex integrations
2. **Reliability** - Playwright is mature and well-tested
3. **Debugging** - Excellent debugging tools with trace viewer
4. **Flexibility** - Can test any Storybook story without special setup
5. **Performance** - Fast test execution with parallel runs
6. **Reporting** - Built-in HTML reporter with detailed traces

## Migration Verification

After implementation:
1. Run `npm install` to install Playwright
2. Run `npx playwright install` to install browsers
3. Run `npm run test-storybook` to execute tests
4. Verify all tests pass
5. Check that Storybook starts automatically
6. Verify test reports are generated correctly

## Risks and Considerations

1. **Storybook Startup Time** - Storybook needs to be running before tests start
2. **Port Conflicts** - Need to ensure port 6006 is available
3. **Browser Installation** - Playwright browsers need to be installed in CI
4. **Test Stability** - Browser-based tests can be flaky if not written carefully

## Success Criteria

- [ ] All Vitest-related files are removed
- [ ] Playwright is installed and configured
- [ ] Tests run successfully against running Storybook
- [ ] Test reports are generated correctly
- [ ] CI/CD pipeline can run tests
- [ ] Documentation is updated

## References

- [Playwright Documentation](https://playwright.dev/)
- [Storybook Documentation](https://storybook.js.org/)
- [Playwright Test Runner](https://playwright.dev/docs/test-intro)
