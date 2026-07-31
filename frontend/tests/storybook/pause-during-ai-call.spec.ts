// Covers tests/cases/pause-abort/pause-during-ai-call.md
import { test, expect } from '@playwright/test';

const PENDING_BACKGROUND = 'rgb(245, 245, 245)';

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

    // Step 11. System dispatches queueMessage action, optimistically adding a pending message
    const pendingMessage11 = page.locator('[data-pending="true"][data-message-content="Please continue later"]');
    await expect(pendingMessage11).toBeVisible();

    // Verify exactly one copy of the message exists (no optimistic/daemon duplicate)
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    // Verify the pending message is a user message that is not yet committed
    await expect(pendingMessage11).toHaveAttribute('data-role', 'user');

    // Verify the pending message is rendered with a gray background
    await expect(pendingMessage11).toHaveCSS('background-color', PENDING_BACKGROUND);

    // Verify the loader of the interrupted call comes BEFORE the pending message while paused
    const loader11 = page.locator('[data-testid="streaming-message"]');
    await expect(loader11).toBeVisible();
    const loaderBox11 = await loader11.boundingBox();
    const pendingBox11 = await pendingMessage11.boundingBox();
    expect(loaderBox11).not.toBeNull();
    expect(pendingBox11).not.toBeNull();
    expect(pendingBox11!.y).toBeGreaterThan(loaderBox11!.y);

    // Step 12. User clicks "click resume" step button
    const stepButton5 = page.locator('[data-testid="step-button-5"]');
    await stepButton5.click();

    // Step 13. System dispatches resumeChat action
    // Verify UI updates: pause/abort buttons visible again, resume/queue buttons hidden
    await expect(page.locator('[data-testid="pause-button"]')).toBeVisible();
    await expect(page.locator('[data-testid="resume-button"]')).not.toBeVisible();

    // The interrupted call has not produced its result yet, so the message stays
    // queued: still gray, still below the loader of that interrupted call
    const pendingMessage13 = page.locator('[data-pending="true"][data-message-content="Please continue later"]');
    await expect(pendingMessage13).toBeVisible();
    await expect(pendingMessage13).toHaveAttribute('data-pending', 'true');
    await expect(pendingMessage13).toHaveCSS('background-color', PENDING_BACKGROUND);
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    const loader13 = page.locator('[data-testid="streaming-message"]');
    await expect(loader13).toBeVisible();
    const loaderBox13 = await loader13.boundingBox();
    const pendingBox13 = await pendingMessage13.boundingBox();
    expect(loaderBox13).not.toBeNull();
    expect(pendingBox13).not.toBeNull();
    expect(pendingBox13!.y).toBeGreaterThan(loaderBox13!.y);

    // Step 14. User clicks "daemon response: chatResumed" step button
    const stepButton6 = page.locator('[data-testid="step-button-6"]');
    await stepButton6.click();

    // Step 15. System receives chatResumed action, sets isPaused=false, isStreaming=true
    // Verify AI chat loader is visible (interrupted call still in flight)
    const loader15 = page.locator('[data-testid="streaming-message"]');
    await expect(loader15).toBeVisible();

    // The pending message is still awaiting daemon confirmation, so it stays visible
    const pendingMessage15 = page.locator('[data-pending="true"][data-message-content="Please continue later"]');
    await expect(pendingMessage15).toBeVisible();
    await expect(pendingMessage15).toHaveCSS('background-color', PENDING_BACKGROUND);
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    // The loader still represents the interrupted call, which must finish before the
    // daemon drains the queue, so the not-yet-sent message stays AFTER the loader
    const pendingBox15 = await pendingMessage15.boundingBox();
    const loaderBox15 = await loader15.boundingBox();
    expect(pendingBox15).not.toBeNull();
    expect(loaderBox15).not.toBeNull();
    expect(pendingBox15!.y).toBeGreaterThan(loaderBox15!.y);

    // Step 16. User clicks "daemon response: streamFinished" step button
    const stepButton7 = page.locator('[data-testid="step-button-7"]');
    await stepButton7.click();

    // Step 17. System receives chatStreamFinished action
    // The interrupted call has produced its result and the chat is resumed, so the
    // queued message has been handed to the AI: it is now a regular user message
    const firstUserMessage17 = page.locator('[data-role="user"][data-message-content="Hello, can you help me?"]');
    const aiResponse17 = page.locator('[data-role="assistant"][data-message-content="AI response after resume"]');
    const promotedMessage17 = page.locator('[data-role="user"][data-message-content="Please continue later"]');
    const loader17 = page.locator('[data-testid="streaming-message"]');

    await expect(firstUserMessage17).toBeVisible();
    await expect(aiResponse17).toBeVisible();
    await expect(promotedMessage17).toBeVisible();
    await expect(loader17).toBeVisible();

    // The promoted message is no longer pending: no data-pending attribute, no gray background
    await expect(promotedMessage17).not.toHaveAttribute('data-pending');
    await expect(promotedMessage17).not.toHaveCSS('background-color', PENDING_BACKGROUND);
    await expect(page.locator('[data-pending="true"]')).toHaveCount(0);

    // Still exactly one copy of the message
    await expect(page.locator('[data-message-content="Please continue later"]')).toHaveCount(1);

    // Verify order in DOM
    const firstBox17 = await firstUserMessage17.boundingBox();
    const aiBox17 = await aiResponse17.boundingBox();
    const promotedBox17 = await promotedMessage17.boundingBox();
    const loaderBox17 = await loader17.boundingBox();

    expect(firstBox17).not.toBeNull();
    expect(aiBox17).not.toBeNull();
    expect(promotedBox17).not.toBeNull();
    expect(loaderBox17).not.toBeNull();

    expect(firstBox17!.y).toBeLessThan(aiBox17!.y);
    expect(aiBox17!.y).toBeLessThan(promotedBox17!.y);
    expect(promotedBox17!.y).toBeLessThan(loaderBox17!.y);

    // Step 18. User clicks "verify final state" step button
    const stepButton8 = page.locator('[data-testid="step-button-8"]');
    await stepButton8.click();

    // Step 19. System verifies final state: isPaused=false, isStreaming=true
    // Wait for the step to complete and show "All steps completed"
    await expect(page.locator('button:has-text("All steps completed")')).toBeVisible({ timeout: 10000 });
  });
});
