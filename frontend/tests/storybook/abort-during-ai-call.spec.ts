// Covers tests/cases/pause-abort/abort-during-ai-call.md
import { test, expect } from '@playwright/test';

const PENDING_BACKGROUND = 'rgb(245, 245, 245)';

test.describe('Abort During AI Call', () => {
  test('aborts during AI call and resumes without aborted message', async ({ page }) => {
    // Step 1. User navigates to the AbortDuringAiCall story with controls
    await page.goto('/iframe.html?globals=&id=pause-abort-abortduringaicall--with-controls&viewMode=story');

    // Step 2. User clicks setup step button
    const stepButton0 = page.locator('[data-testid="step-button-0"]');
    await stepButton0.click();

    // Step 3. System sets up preconditions: isStreaming=true, isPaused=false, isAborted=false
    // Verify streaming indicator is visible (AI response loader)
    await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();
    // Verify abort button is visible (streaming and not paused)
    await expect(page.locator('[data-testid="abort-button"]')).toBeVisible();

    // Step 4. User clicks "click abort" step button
    const stepButton1 = page.locator('[data-testid="step-button-1"]');
    await stepButton1.click();

    // Step 5. System dispatches abortChat action
    // Pause and abort buttons are disabled (pending state) while waiting for daemon
    await expect(page.locator('[data-testid="pause-button"]')).toBeDisabled();
    await expect(page.locator('[data-testid="abort-button"]')).toBeDisabled();

    // Step 6. User clicks "daemon response: streamAborted" step button
    const stepButton2 = page.locator('[data-testid="step-button-2"]');
    await stepButton2.click();

    // Step 7. System receives streamAborted action, sets isPaused=true, isAborted=true, isStreaming=false
    // The AI call was cancelled, so the loader is NOT visible (unlike pause flow)
    await expect(page.locator('[data-testid="streaming-message"]')).not.toBeVisible();
    // Verify resume button is visible (isPaused || isAborted)
    await expect(page.locator('[data-testid="resume-button"]')).toBeVisible();
    // Verify abort/pause buttons are NOT visible (isStreaming=false)
    await expect(page.locator('[data-testid="abort-button"]')).not.toBeVisible();
    await expect(page.locator('[data-testid="pause-button"]')).not.toBeVisible();

    // Step 8. User clicks "type message" step button
    const stepButton3 = page.locator('[data-testid="step-button-3"]');
    await stepButton3.click();

    // Step 9. System types message in input field
    const messageInput = page.locator('[data-testid="message-input"]');
    await expect(messageInput).toHaveValue('Please continue later');

    // Step 10. User clicks "click send (queue)" step button
    const stepButton4 = page.locator('[data-testid="step-button-4"]');
    await stepButton4.click();

    // Step 11. System dispatches queueMessage action, optimistically adding a pending message
    const pendingMessage11 = page.locator('[data-pending="true"][data-message-content="Please continue later"]');
    await expect(pendingMessage11).toBeVisible();

    // Verify exactly one copy of the message exists (no optimistic/daemon duplicate)
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    // Verify the pending message is a user message that is not yet committed
    await expect(pendingMessage11).toHaveAttribute('data-role', 'user');

    // Verify the pending message is rendered with a gray background
    await expect(pendingMessage11).toHaveCSS('background-color', PENDING_BACKGROUND);

    // No loader above the pending message (the AI call was cancelled on abort)
    await expect(page.locator('[data-testid="streaming-message"]')).not.toBeVisible();

    // Step 12. User clicks "click resume" step button
    const stepButton5 = page.locator('[data-testid="step-button-5"]');
    await stepButton5.click();

    // Step 13. System dispatches resumeChat action
    // Resume button is disabled (pending state) while waiting for daemon
    await expect(page.locator('[data-testid="resume-button"]')).toBeVisible();
    await expect(page.locator('[data-testid="resume-button"]')).toBeDisabled();
    // Queue button is disabled (isResumePending)
    await expect(page.locator('[data-testid="send-button"]')).toBeDisabled();
    // Only resume and queue buttons are visible
    await expect(page.locator('[data-testid="pause-button"]')).not.toBeVisible();
    await expect(page.locator('[data-testid="abort-button"]')).not.toBeVisible();

    // Step 14. User clicks "daemon response: chatResumed" step button
    const stepButton6 = page.locator('[data-testid="step-button-6"]');
    await stepButton6.click();

    // Step 15. System receives chatResumed action, sets isPaused=false, isAborted=false, isStreaming=true
    // The aborted AI call was cancelled, so the queued message is promoted immediately
    const promotedMessage15 = page.locator('[data-role="user"][data-message-content="Please continue later"]');
    await expect(promotedMessage15).toBeVisible();
    // The promoted message is no longer pending: no data-pending attribute, no gray background
    await expect(promotedMessage15).not.toHaveAttribute('data-pending');
    await expect(promotedMessage15).not.toHaveCSS('background-color', PENDING_BACKGROUND);
    await expect(page.locator('[data-pending="true"]')).toHaveCount(0);
    // Still exactly one copy of the message
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    // A new AI call starts for the promoted message, so the loader is visible AFTER it
    const loader15 = page.locator('[data-testid="streaming-message"]');
    await expect(loader15).toBeVisible();

    // DOM order: original user → promoted user → loader
    const originalUser15 = page.locator('[data-role="user"][data-message-content="Hello, can you help me?"]');
    const originalBox15 = await originalUser15.boundingBox();
    const promotedBox15 = await promotedMessage15.boundingBox();
    const loaderBox15 = await loader15.boundingBox();
    expect(originalBox15).not.toBeNull();
    expect(promotedBox15).not.toBeNull();
    expect(loaderBox15).not.toBeNull();
    expect(originalBox15!.y).toBeLessThan(promotedBox15!.y);
    expect(promotedBox15!.y).toBeLessThan(loaderBox15!.y);

    // Step 16. User clicks "verify final state" step button
    const stepButton7 = page.locator('[data-testid="step-button-7"]');
    await stepButton7.click();

    // Step 17. System verifies final state: isPaused=false, isStreaming=true
    // Wait for the step to complete and show "All steps completed"
    await expect(page.locator('button:has-text("All steps completed")')).toBeVisible({ timeout: 10000 });
  });
});
