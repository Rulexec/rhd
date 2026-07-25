# Storybook

## Overview

Storybook is used for developing and testing UI components in isolation. Stories with interactive controls allow step-by-step testing of complex user flows.

## Configuration

- **Config file**: `frontend/.storybook/main.ts`
- **Port**: 6006 (default)
- **Test runner**: Playwright (direct browser testing)
- **Test location**: `frontend/tests/storybook/`

## Running Storybook

```bash
# Start Storybook dev server
cd frontend && npm run storybook

# Run Storybook tests (auto-starts Storybook)
cd frontend && npm run test-storybook

# Run tests with UI mode (for users only, not for agents)
cd frontend && npm run test-storybook:ui

# Debug tests
cd frontend && npm run test-storybook:debug
```

## Creating Stories

### Basic Structure

```typescript
import type { Meta, StoryObj } from '@storybook/svelte';
import Component from './Component.svelte';

const meta = {
  title: 'Category/ComponentName',
  component: Component,
  parameters: {
    layout: 'fullscreen',
  },
  decorators: [
    (Story) => {
      // Setup stores/state
      return Story();
    },
  ],
} satisfies Meta<Component>;

export default meta;
type Story = StoryObj<typeof meta>;

export const StoryName: Story = {
  // Story configuration
};
```

### Interactive Stories with Step Controls

For testing complex flows, use `StoryExecutionControls` component:

```typescript
import ChatViewWithControls from './ChatViewWithControls.svelte';
import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';

const storyDefinition: StoryControlDefinition<StoryState> = {
  getInitialState: () => ({ /* initial state */ }),
  steps: [
    {
      name: 'Step name',
      execute: async ({ state }) => {
        // Perform action
        return { state: { ...state, updated: true } };
      },
    },
  ],
};

export const WithControls: Story = {
  render: () => ({
    Component: ChatViewWithControls,
    props: { story: storyDefinition },
  }),
};
```

### Test IDs

Add `data-testid` attributes to elements that need to be targeted by tests:

```typescript
// frontend/src/stories/testIds.ts
export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
} as const;

export const STEP_BUTTON_PREFIX = 'step-button';

export function stepButtonTestId(stepIndex: number): string {
  return `${STEP_BUTTON_PREFIX}-${stepIndex}`;
}
```

Use in components:

```svelte
<button data-testid={stepButtonTestId(currentStepIndex)}>
  Execute Step
</button>
```

## Story Guidelines

### What to Include

1. **Only stories used in tests**: Do not create default or documentation-only stories. Every story should have a corresponding test that verifies its behavior.

2. **Interactive controls for complex flows**: Use `StoryExecutionControls` for stories that need step-by-step execution to verify state changes.

3. **Mock external dependencies**: Use mock WebSocket handlers or other mocks to simulate backend behavior.

4. **State reset in decorators**: Always reset stores/state in decorators to ensure test isolation.

### What to Avoid

1. **Default stories without tests**: Do not create `export const Default: Story = {}` unless it has a corresponding test.

2. **Autodocs tags**: Remove `tags: ['autodocs']` unless documentation is specifically needed.

3. **Exported story definitions**: Keep `storyDefinition` as a local `const`, not `export const`, unless needed by other files.

4. **Multiple similar stories**: If stories are similar, consolidate into one story with controls rather than creating multiple variants.

## Writing Storybook Tests

### Test Structure

```typescript
// Covers test-case-name.md steps X-Y
import { test, expect } from '@playwright/test';

test.describe('Component Storybook Tests', () => {
  test('story name', async ({ page }) => {
    // Step 1. Navigate to story
    await page.goto('/iframe.html?id=category-component--story-name&viewMode=story');
    
    // Step 2. Interact with story
    await page.locator('[data-testid="element"]').click();
    
    // Step 3. Verify state
    await expect(page.locator('text=Expected text')).toBeVisible();
  });
});
```

### Test Guidelines

1. **Step comments**: Follow [test case format](test-cases.md) with step comments
2. **Test IDs**: Use `data-testid` attributes for reliable element selection
3. **Auto-waiting**: Playwright auto-waits for elements, but use explicit waits for async operations
4. **Story URL format**: `/iframe.html?id=<category>-<component>--<story-name>&viewMode=story`
5. **Agent restrictions**: Do not run `npm run test-storybook:ui` as an agent. This command opens an interactive UI and should only be used by users for debugging.

### Example Test

```typescript
// Covers storybook-chat-interaction.md steps 1-14
test('step execution flow', async ({ page }) => {
  // Step 1. User navigates to the ChatView story with controls
  await page.goto('/iframe.html?id=pause-abort-chatview--with-controls&viewMode=story');

  // Step 2. User clicks step 1 button
  const stepButton0 = page.locator('[data-testid="step-button-0"]');
  await stepButton0.click();

  // Steps 3-7. System processes and user message appears
  await expect(page.locator('text=Hello, how are you?')).toBeVisible();

  // Step 8. Pause and Abort buttons become visible
  await expect(page.locator('text=Pause')).toBeVisible();
});
```

## Mock WebSocket

For stories that need to simulate WebSocket communication:

```typescript
import { setMockWsHandler, emitWsEvent, clearMockWsHandlers } from '@/stories/mockWs';

// Set up handler for specific message type
setMockWsHandler('sendMessage', async (request) => {
  return { type: 'response', id: request.id, success: true, data: {} };
});

// Emit WebSocket events
emitWsEvent('chatStreamChunk', { chatId: 1, content: 'AI response' });
emitWsEvent('chatStreamFinished', { chatId: 1 });

// Clean up handlers
clearMockWsHandlers();
```

## File Structure

```
frontend/
├── .storybook/
│   └── main.ts              # Storybook configuration
├── src/stories/
│   ├── testIds.ts           # Test ID constants and helpers
│   ├── testUtils.ts         # Test utilities (sleep, etc.)
│   ├── mockWs.ts            # Mock WebSocket implementation
│   ├── StoryExecutionControls.svelte  # Step execution UI
│   └── <category>/
│       ├── Component.stories.ts       # Story definitions
│       └── ComponentWithControls.svelte  # Wrapper with controls
├── tests/storybook/
│   └── component.spec.ts    # Playwright tests
└── playwright.config.ts     # Playwright configuration
```

## Playwright Configuration

```typescript
// frontend/playwright.config.ts
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

## Troubleshooting

### Story not found
- Check story URL format: `/iframe.html?id=<category>-<component>--<story-name>&viewMode=story`
- Story names are kebab-case: `WithControls` → `with-controls`
- Category names are kebab-case: `Pause-Abort` → `pause-abort`

### Elements not found
- Ensure `data-testid` attributes are set correctly
- Use Playwright's auto-waiting: `await expect(locator).toBeVisible()`
- Check if story has fully loaded before interacting

### Tests flaky
- Add explicit waits for async operations
- Use `await expect()` for assertions that need to wait
- Increase timeout for slow operations: `await expect(locator).toBeVisible({ timeout: 10000 })`

## References

- [Test Cases](test-cases.md) - Test case format and step comments
- [Frontend E2E Testing](frontend-e2e.md) - E2E test infrastructure
- [Playwright Documentation](https://playwright.dev/)
- [Storybook Documentation](https://storybook.js.org/)
