---
name: storybook-test
description: Guides test-driven updates to Storybook tests and frontend implementation when users report issues after executing story steps. Use when test assertions and component implementation both need updating.
disable-model-invocation: true
---

# Storybook Test-Driven Development Skill

## Purpose

This skill guides agents through updating Storybook tests and frontend implementation when users report issues after executing story steps. It follows a test-first approach: update test assertions to verify new requirements, then update implementation to make tests pass.

## When to Use

Activate this skill when:
- User reports visual or behavioral issues after executing specific story steps
- User describes what they see vs. what they expect to see
- Changes are needed to both test assertions and component implementation
- Iterative refinement is required to achieve correct behavior

## Workflow

### 1. Understand the Issue

Listen to the user's description:
- Which step(s) they executed
- What they observed after the step
- What they expected to see instead

Example input:
> "After clicking 'send message', the message appears before the loader, but it should appear after."

### 2. Update Test Assertions First

Before changing any implementation:
- Identify the test file for the story (typically in `frontend/tests/storybook/`)
- Update test assertions to verify the new expected behavior
- Use data attributes (`data-testid`, `data-role`, etc.) instead of class names or text matching
- Add assertions for ordering, visibility, styling, or state as needed

**Why test first?** Tests define the contract. If tests pass but behavior is wrong, the tests are incomplete. By writing tests first, you ensure the implementation meets the actual requirements.

### 3. Run Tests to See Failures

Run the specific test to confirm it fails with the current implementation:

```bash
cd frontend && npm run test-storybook -- --grep "test name"
```

Use `--grep` to filter and run only the specific test being worked on. This speeds up the feedback loop.

### 4. Update Implementation

Make the minimal changes needed to make the tests pass:
- Update component markup, styles, or logic
- Add data attributes if tests require them
- Reorder rendering if tests check element order
- Update state management if tests check store values

### 5. Iterate Until Tests Pass

Run tests again:
- If tests pass: verify the behavior matches user expectations
- If tests fail: analyze the failure, adjust implementation (or tests if requirements need clarification)
- Repeat until all assertions pass

### 6. Verify Full Test Suite

Once the specific test passes, run the full Storybook test suite to ensure no regressions:

```bash
cd frontend && npm run test-storybook
```

## Best Practices

### Test Assertions

- **Use data attributes**: Prefer `data-testid`, `data-role`, `data-message-content` over class names or text matching
- **Check ordering**: Use `boundingBox()` to verify element positions when order matters
- **Check visibility**: Use `toBeVisible()` and `not.toBeVisible()` for conditional elements
- **Check styling**: Use `toHaveCSS()` for visual properties like opacity, color, etc.
- **Check state**: Use `toHaveAttribute()` for disabled states, aria attributes, etc.

### Implementation Changes

- **Minimal changes**: Only change what's needed to make tests pass
- **Preserve existing behavior**: Don't break other tests or functionality
- **Add data attributes**: When tests need to target elements, add appropriate data attributes
- **Update styles**: When visual appearance needs to change, update CSS in the component

### Test Filtering

Always use `--grep` to run only the specific test during development:

```bash
# Run only tests matching "pause during AI call"
cd frontend && npm run test-storybook -- --grep "pause during AI call"

# Run only tests matching a specific test name
cd frontend && npm run test-storybook -- --grep "pauses during AI call and resumes"
```

This speeds up the feedback loop from minutes to seconds.

## Example Scenario

**User reports:**
> "After clicking 'send message', the queued message appears before the loader, but it should appear after the loader."

**Agent workflow:**

1. **Update test** to check that loader appears before queued message:
   ```typescript
   const loader = page.locator('[data-testid="streaming-message"]');
   const queuedMessage = page.locator('[data-role="queued"]');
   const loaderBox = await loader.boundingBox();
   const queuedBox = await queuedMessage.boundingBox();
   expect(loaderBox!.y).toBeLessThan(queuedBox!.y);
   ```

2. **Run test** to see it fail:
   ```bash
   cd frontend && npm run test-storybook -- --grep "pause during AI call"
   ```

3. **Update implementation** in `MessageList.svelte` to render loader before queued messages:
   ```svelte
   {#if ($isStreaming || $isPaused) && !$streamingMessageId}
     <StreamingMessage ... />
   {/if}
   {#each $queuedMessages as message}
     <Message {message} />
   {/each}
   ```

4. **Run test again** to verify it passes.

5. **Run full suite** to ensure no regressions.

## Common Patterns

### Checking Element Order

```typescript
const element1 = page.locator('[data-testid="element-1"]');
const element2 = page.locator('[data-testid="element-2"]');
const box1 = await element1.boundingBox();
const box2 = await element2.boundingBox();
expect(box1!.y).toBeLessThan(box2!.y); // element1 appears before element2
```

### Checking Disabled State

```typescript
const button = page.locator('[data-testid="pause-button"]');
await expect(button).toBeDisabled();
await expect(button).toHaveAttribute('disabled', '');
```

### Checking CSS Properties

```typescript
const element = page.locator('[data-role="queued"]');
await expect(element).toHaveCSS('opacity', '0.8');
```

### Checking Visibility

```typescript
const indicator = page.locator('[data-testid="queued-indicator"]');
await expect(indicator).not.toBeVisible(); // Should be hidden
```

## Troubleshooting

### Test passes but behavior is wrong

The test assertions are incomplete. Add more specific checks:
- Check element order, not just visibility
- Check CSS properties, not just presence
- Check state attributes, not just text content

### Test fails but behavior seems correct

The test might be checking the wrong thing:
- Verify the selector is correct
- Check if the element exists in the DOM
- Ensure the test is waiting for the right condition

### Multiple tests failing

You may have broken existing behavior:
- Check if your changes affect other stories
- Ensure data attributes are unique
- Verify state management changes don't break other flows

## Related Files

- Test files: `frontend/tests/storybook/*.spec.ts`
- Story files: `frontend/src/stories/**/*.stories.ts`
- Component files: `frontend/src/components/*.svelte`
- Test IDs: `frontend/src/stories/testIds.ts`
- Memory: `memory/storybook.md`
