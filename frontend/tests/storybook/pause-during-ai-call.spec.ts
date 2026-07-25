// Covers tests/cases/pause-abort/pause-during-ai-call.md
import { test, expect } from '@playwright/test';

test.describe('Pause During AI Call', () => {
  test('pauses during AI call and resumes with pending tool calls', async ({ page }) => {
    // Step 1. User navigates to the PauseDuringAiCall story with controls
    await page.goto('/iframe.html?globals=&id=pause-abort-pauseduringaicall--with-controls&viewMode=story');

    // Step 2. User clicks setup step button
    const stepButton0 = page.locator('[data-testid="step-button-0"]');
    await stepButton0.click();

    // Step 3. System sets up preconditions: isStreaming=true, isPaused=false
    // Verify streaming indicator is visible (AI response loader)
    await expect(page.locator('.message.assistant.streaming')).toBeVisible();

    // Step 4. User clicks "click pause" step button
    const stepButton1 = page.locator('[data-testid="step-button-1"]');
    await stepButton1.click();

    // Step 5. System dispatches pauseChat action
    // Verify pause button is disabled (UI indicator that pause is being applied)
    const pauseButton = page.locator('[data-testid="pause-button"]');
    await expect(pauseButton).toBeDisabled();

    // Step 6. User clicks "daemon response: chatPaused" step button
    const stepButton2 = page.locator('[data-testid="step-button-2"]');
    await stepButton2.click();

    // Step 7. System receives chatPaused action, sets isPaused=true, isStreaming=false
    // Verify AI response loader is still present (we're still waiting for AI response)
    await expect(page.locator('.message.assistant.streaming')).toBeVisible();
    // Verify resume button is visible
    await expect(page.locator('[data-testid="resume-button"]')).toBeVisible();

    // Step 8. User clicks "type message" step button
    const stepButton3 = page.locator('[data-testid="step-button-3"]');
    await stepButton3.click();

    // Step 9. System types message in input field
    const messageInput = page.locator('[data-testid="message-input"]');
    await expect(messageInput).toHaveValue('Please continue later');

    // Step 10. User clicks "click send (queue)" step button
    const stepButton4 = page.locator('[data-testid="step-button-4"]');
    await stepButton4.click();

    // Step 11. System dispatches queueMessage action
    // Verify user message is added to chat optimistically
    await expect(page.locator('text=Please continue later')).toBeVisible();

    // Step 12. User clicks "daemon response: messageQueued" step button
    const stepButton5 = page.locator('[data-testid="step-button-5"]');
    await stepButton5.click();

    // Step 13. System receives messageQueued action, adds message to queuedMessages store
    // Verify queued indicator is visible
    await expect(page.locator('text=1 message queued')).toBeVisible();

    // Step 14. User clicks "click resume" step button
    const stepButton6 = page.locator('[data-testid="step-button-6"]');
    await stepButton6.click();

    // Step 15. System dispatches resumeChat action
    // Verify UI updates: pause/abort buttons visible again, resume/queue buttons hidden
    await expect(page.locator('[data-testid="pause-button"]')).toBeVisible();
    await expect(page.locator('[data-testid="resume-button"]')).not.toBeVisible();

    // Step 16. User clicks "daemon response: chatResumed" step button
    const stepButton7 = page.locator('[data-testid="step-button-7"]');
    await stepButton7.click();

    // Step 17. System receives chatResumed action, sets isPaused=false, isStreaming=true
    // Verify AI chat loader is visible
    await expect(page.locator('.message.assistant.streaming')).toBeVisible();
    // Verify queued user message is visible under loading
    await expect(page.locator('text=Please continue later')).toBeVisible();

    // Step 18. User clicks "daemon response: streamFinished" step button
    const stepButton8 = page.locator('[data-testid="step-button-8"]');
    await stepButton8.click();

    // Step 19. System receives chatStreamFinished action
    // Verify AI final response is visible
    await expect(page.locator('text=AI response after resume')).toBeVisible();
    // Verify user message is still visible
    await expect(page.locator('text=Please continue later')).toBeVisible();

    // Step 20. User clicks "verify final state" step button
    const stepButton9 = page.locator('[data-testid="step-button-9"]');
    await stepButton9.click();

    // Step 21. System verifies final state: isPaused=false, isStreaming=false
    // Wait for the step to complete and show "All steps completed"
    await expect(page.locator('button:has-text("All steps completed")')).toBeVisible({ timeout: 10000 });
  });
});
