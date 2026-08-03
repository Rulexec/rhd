# Storybook Vitest Migration Plan

**Status: NOT IMPLEMENTED**

This plan was not implemented. The Vitest-based approach was abandoned in favor of direct Playwright testing.

**Implemented by:** [storybook-playwright-direct.md](./storybook-playwright-direct.md)

---

# Storybook Vitest Migration Plan

## Goal

Replace `@storybook/test-runner` (Jest + Playwright) with `@storybook/addon-vitest` for running Storybook interaction tests.

## Current State

### Dependencies
- `@storybook/test-runner`: ^0.23.0
- `@storybook/test`: ^8.6.15
- `@storybook/addon-interactions`: ^8.6.14
- Jest + Playwright setup

### Configuration Files (WILL BE REMOVED)
- `.storybook/test-runner.ts` - Test runner configuration → **DELETE**
- `jest.config.storybook.js` - Jest configuration for Storybook tests → **DELETE**

### Test Files
- `frontend/src/stories/pause-abort/ChatView.stories.test.ts` - Jest/Playwright test

### Scripts
- `test-storybook`: Runs `test-storybook` (from @storybook/test-runner)
- `test-storybook:custom`: Runs `jest --config jest.config.storybook.js`

## Migration Steps

### Step 1: Update package.json Dependencies

**Remove:**
```json
"@storybook/test-runner": "^0.23.0"
```

**Add:**
```json
"@storybook/addon-vitest": "^8.6.0"
```

**Keep:**
- `@storybook/test`: ^8.6.15 (still needed for test utilities)
- `@storybook/addon-interactions`: ^8.6.14 (still needed for interaction testing)

**Remove (no longer needed):**
- `@types/jest`: ^30.0.0 (if only used for Storybook tests)
- `ts-node`: ^10.9.2 (if only used for Jest)

### Step 2: Update .storybook/main.ts

Add `@storybook/addon-vitest` to the addons array:

```typescript
const config: StorybookConfig = {
  stories: ['../src/**/*.mdx', '../src/**/*.stories.@(js|ts|svelte)'],
  addons: [
    '@storybook/addon-links',
    '@storybook/addon-essentials',
    '@storybook/addon-interactions',
    '@storybook/addon-vitest', // Add this
  ],
  // ... rest of config
};
```

### Step 3: Remove Obsolete Configuration Files

**Delete these files:**
- `.storybook/test-runner.ts` - No longer needed
- `jest.config.storybook.js` - No longer needed

### Step 4: Migrate Test File

**Current test:** `frontend/src/stories/pause-abort/ChatView.stories.test.ts`

**Migration approach:**

The `@storybook/addon-vitest` provides two testing modes:
1. **In-browser testing** - Tests run inside the browser with Storybook context
2. **Node.js testing** - Tests run in Node.js with Vitest

For interaction tests that need DOM access, use in-browser testing with `@storybook/test`.

**New test structure:**

```typescript
// frontend/src/stories/pause-abort/ChatView.stories.test.ts
import { expect, test } from '@storybook/test';
import { meta, WithControls } from './ChatView.stories';

test('ChatView WithControls - step execution flow', async () => {
  // Render the story
  const { container } = await WithControls.render();
  
  // Wait for step button to appear
  const stepButton0 = container.querySelector('[data-testid="step-button-0"]');
  expect(stepButton0).toBeTruthy();
  
  // Click first step
  stepButton0?.dispatchEvent(new Event('click'));
  
  // Wait for user message to appear
  await expect(container).toContainText('Hello, how are you?');
  
  // Verify Pause and Abort buttons are visible
  await expect(container).toContainText('Pause');
  await expect(container).toContainText('Abort');
  
  // Click second step
  const stepButton1 = container.querySelector('[data-testid="step-button-1"]');
  stepButton1?.dispatchEvent(new Event('click'));
  
  // Wait for AI response
  await expect(container).toContainText("I'm doing well, thank you!");
  
  // Verify no streaming dots
  const streamingDots = container.querySelectorAll('.streaming-dots');
  expect(streamingDots.length).toBe(0);
  
  // Verify send button is enabled
  const sendButton = container.querySelector('[data-testid="send-button"]') as HTMLButtonElement;
  expect(sendButton?.disabled).toBe(false);
});
```

### Step 5: Update package.json Scripts

**Remove:**
```json
"test-storybook": "test-storybook",
"test-storybook:custom": "jest --config jest.config.storybook.js"
```

**Add:**
```json
"test-storybook": "vitest run --config vitest.config.storybook.ts"
```

### Step 6: Create Vitest Configuration for Storybook

**Create:** `frontend/vitest.config.storybook.ts`

```typescript
import { defineConfig } from 'vitest/config';
import { resolve } from 'path';
import { aliases } from './aliases.ts';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['src/stories/**/*.test.ts'],
    globals: true,
    setupFiles: ['./src/stories/vitest.setup.ts'],
  },
  resolve: {
    alias: {
      ...aliases,
    },
  },
});
```

**Create:** `frontend/src/stories/vitest.setup.ts`

```typescript
import '@testing-library/jest-dom';
```

### Step 7: Update Dependencies (if needed)

**Add:**
```json
"@testing-library/jest-dom": "^6.9.1" // Already present
```

**Remove (if only used for Storybook):**
```json
"@types/jest": "^30.0.0"
"ts-node": "^10.9.2"
```

## File Changes Summary

| File | Action | Description |
|------|--------|-------------|
| `frontend/package.json` | Modify | Remove test-runner, add addon-vitest, update scripts |
| `frontend/.storybook/main.ts` | Modify | Add @storybook/addon-vitest to addons |
| `frontend/.storybook/test-runner.ts` | **DELETE** | No longer needed |
| `frontend/jest.config.storybook.js` | **DELETE** | No longer needed |
| `frontend/src/stories/pause-abort/ChatView.stories.test.ts` | Modify | Migrate from Jest/Playwright to Vitest |
| `frontend/vitest.config.storybook.ts` | Create | Vitest configuration for Storybook tests |
| `frontend/src/stories/vitest.setup.ts` | Create | Vitest setup file |

## Testing Approach

### Separate Vitest Config (Recommended)

Keep Storybook tests separate from unit/e2e tests:
- `vitest.config.unit.ts` - Unit tests
- `vitest.config.e2e.ts` - E2E tests
- `vitest.config.storybook.ts` - Storybook interaction tests

## Migration Verification

After implementation:
1. Run `npm install` to update dependencies
2. Run `npm run storybook` to start Storybook
3. Run `npm run test-storybook` to execute tests
4. Verify all tests pass
5. Check that interaction tests work correctly

## Risks and Considerations

1. **API Differences**: `@storybook/addon-vitest` may have different APIs than `@storybook/test-runner`. Verify the exact API before implementation.

2. **Browser vs Node**: Decide whether tests should run in browser (with Storybook context) or Node.js (faster but less realistic).

3. **Playwright Dependency**: If browser testing is needed, `@storybook/addon-vitest` may use Playwright or a different browser automation tool.

4. **Test Compatibility**: Some Jest/Playwright APIs may not have direct Vitest equivalents. May need to refactor test logic.

5. **CI/CD**: Update CI pipelines to use the new test command.

## Success Criteria

- [ ] `@storybook/test-runner` is removed from dependencies
- [ ] `@storybook/addon-vitest` is installed and configured
- [ ] `.storybook/test-runner.ts` is deleted
- [ ] `jest.config.storybook.js` is deleted
- [ ] Test file is migrated to Vitest syntax
- [ ] Tests pass with `npm run test-storybook`
- [ ] No Jest/Playwright dependencies remain (if not used elsewhere)
- [ ] Documentation is updated (if any)

## References

- [Storybook Vitest Addon Documentation](https://storybook.js.org/docs/api/addons/addon-vitest)
- [Vitest Documentation](https://vitest.dev/)
- [Storybook Test Documentation](https://storybook.js.org/docs/writing-tests)
