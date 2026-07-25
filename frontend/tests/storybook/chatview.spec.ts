// Covers storybook-chat-interaction.md steps 1-14
import { test, expect } from '@playwright/test';

test.describe('ChatView Storybook Tests', () => {
  test('step execution flow', async ({ page }) => {
    // Step 1. User navigates to the ChatView story with controls
    await page.goto('/iframe.html?id=pause-abort-chatview--with-controls&viewMode=story');

    // Step 2. User clicks step 1 button ("Add user message")
    const stepButton0 = page.locator('[data-testid="step-button-0"]');
    await expect(stepButton0).toBeVisible();
    await stepButton0.click();

    // Steps 3-5. System sets up mock handler, types message, clicks Send
    // Step 6. System dispatches sendMessage action
    // Step 7. User message appears in chat
    await expect(page.locator('text=Hello, how are you?')).toBeVisible();

    // Step 8. Pause and Abort buttons become visible (streaming state)
    await expect(page.locator('text=Pause')).toBeVisible();
    await expect(page.locator('text=Abort')).toBeVisible();

    // Step 9. User clicks step 2 button ("Receive AI response")
    const stepButton1 = page.locator('[data-testid="step-button-1"]');
    await stepButton1.click();

    // Steps 10-11. System emits chatStreamChunk and chatStreamFinished events
    // Step 12. AI message appears in chat with full content
    await expect(page.locator('text=I\'m doing well, thank you!')).toBeVisible();

    // Step 13. Streaming dots disappear
    const streamingDots = page.locator('.streaming-dots');
    await expect(streamingDots).toHaveCount(0);

    // Step 14. Send button becomes disabled (input is empty)
    const sendButton = page.locator('[data-testid="send-button"]');
    await expect(sendButton).toBeDisabled();
  });
});
