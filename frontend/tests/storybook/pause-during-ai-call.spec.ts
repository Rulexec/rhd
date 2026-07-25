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
    await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();

    // Step 4. User clicks "click pause" step button
    const stepButton1 = page.locator('[data-testid="step-button-1"]');
    await stepButton1.click();

    // Step 5. System dispatches pauseChat action
    // Verify pause button is disabled (UI indicator that pause is being applied)
    const pauseButton = page.locator('[data-testid="pause-button"]');
    await expect(pauseButton).toBeDisabled();
    // Verify pause button has disabled attribute
    await expect(pauseButton).toHaveAttribute('disabled', '');

    // Step 6. User clicks "daemon response: chatPaused" step button
    const stepButton2 = page.locator('[data-testid="step-button-2"]');
    await stepButton2.click();

    // Step 7. System receives chatPaused action, sets isPaused=true, isStreaming=false
    // Verify AI response loader is still present (we're still waiting for AI response)
    await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();
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
    // (message is sent to daemon but not yet in queuedMessages store)

    // Step 12. User clicks "daemon response: messageQueued" step button
    const stepButton5 = page.locator('[data-testid="step-button-5"]');
    await stepButton5.click();

    // Step 13. System receives messageQueued action, adds message to queuedMessages store
    // Verify queued message appears after loader, not before
    const loader13 = page.locator('[data-testid="streaming-message"]');
    const queuedMessage13 = page.locator('[data-role="queued"][data-message-content="Please continue later"]');
    await expect(queuedMessage13).toBeVisible();
    // Verify loader comes before queued message in DOM
    await expect(loader13).toBeVisible();
    const loaderBox13 = await loader13.boundingBox();
    const queuedBox13 = await queuedMessage13.boundingBox();
    expect(loaderBox13).not.toBeNull();
    expect(queuedBox13).not.toBeNull();
    expect(queuedBox13!.y).toBeGreaterThan(loaderBox13!.y);
    // Verify queued user message has reduced opacity
    await expect(queuedMessage13).toHaveCSS('opacity', '0.8');
    // Verify queued indicator is NOT visible (removed)
    await expect(page.locator('[data-testid="queued-indicator"]')).not.toBeVisible();

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
    // Verify AI chat loader is visible (new stream started)
    await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();
    // Verify queued messages are cleared (sent to daemon)
    await expect(page.locator('[data-queued-messages-count]')).not.toBeVisible();

    // Step 18. User clicks "daemon response: streamFinished" step button
    const stepButton8 = page.locator('[data-testid="step-button-8"]');
    await stepButton8.click();

    // Step 19. System receives chatStreamFinished action
    // Verify message order: first user message, AI response, queued message, loader
    const firstUserMessage19 = page.locator('[data-role="user"][data-message-content="Hello, can you help me?"]');
    const aiResponse19 = page.locator('[data-role="assistant"][data-message-content="AI response after resume"]');
    const queuedMessage19 = page.locator('[data-role="queued"][data-message-content="Please continue later"]');
    const loader19 = page.locator('[data-testid="streaming-message"]');
    
    await expect(firstUserMessage19).toBeVisible();
    await expect(aiResponse19).toBeVisible();
    await expect(queuedMessage19).toBeVisible();
    await expect(loader19).toBeVisible();
    
    // Verify order in DOM
    const firstBox19 = await firstUserMessage19.boundingBox();
    const aiBox19 = await aiResponse19.boundingBox();
    const queuedBox19 = await queuedMessage19.boundingBox();
    const loaderBox19 = await loader19.boundingBox();
    
    expect(firstBox19).not.toBeNull();
    expect(aiBox19).not.toBeNull();
    expect(queuedBox19).not.toBeNull();
    expect(loaderBox19).not.toBeNull();
    
    expect(firstBox19!.y).toBeLessThan(aiBox19!.y);
    expect(aiBox19!.y).toBeLessThan(queuedBox19!.y);
    expect(queuedBox19!.y).toBeGreaterThan(loaderBox19!.y);

    // Step 20. User clicks "verify final state" step button
    const stepButton9 = page.locator('[data-testid="step-button-9"]');
    await stepButton9.click();

    // Step 21. System verifies final state: isPaused=false, isStreaming=false
    // Wait for the step to complete and show "All steps completed"
    await expect(page.locator('button:has-text("All steps completed")')).toBeVisible({ timeout: 10000 });
  });
});
