# Auto-Select Model Plan

## Problem

When creating a new chat, the model selector shows available models but none is selected. User must manually pick a model before sending messages. The e2e test documents this as a known bug.

## Solution

Use Svelte's reactive statement to auto-select the first available model when:
- `availableModels` has items AND
- `selectedModel` is null

This reactive approach handles all race conditions automatically:
- Models load before chat creation
- Models load after chat creation
- Existing chat with no saved model

## Implementation

### 1. Add reactive auto-selection in [`MessageInput.svelte`](frontend/src/components/MessageInput.svelte:9)

Add a reactive statement after the imports that watches both stores and auto-selects when needed.

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { isStreaming, streamError, currentChatId, availableModels, selectedModel } from '../lib/chatStores';
  import { sendMessage, abortChat, loadAvailableModels } from '../lib/chatWs';

  let input = '';
  let textareaElement: HTMLTextAreaElement;

  // Auto-select first model when models are loaded and none is selected
  $: if ($availableModels.length > 0 && $selectedModel === null) {
    selectedModel.set($availableModels[0]);
  }

  onMount(() => {
    loadAvailableModels();
  });
  
  // ... rest of component
</script>
```

### 2. Update e2e test in [`chat.test.ts`](frontend/src/tests/e2e/chat.test.ts:66)

Split assertions into separate `waitFor` calls with custom error messages for clearer debugging.

```typescript
it('creates new chat with model pre-selected', async () => {
  if (!window.prompt) {
    (window as any).prompt = () => null;
  }
  vi.spyOn(window, 'prompt').mockReturnValue('Test Chat');

  await waitFor(
    () => {
      const wsState = get(wsConnected);
      if (!wsState) {
        throw new Error('WebSocket not connected');
      }
    },
    { timeout: 5000 }
  );

  render(ChatsTab);

  const newChatButton = screen.getByText('+ New Chat');
  await fireEvent.click(newChatButton);

  // Wait for model select to appear
  await waitFor(
    () => {
      const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
      if (!modelSelect) {
        throw new Error('Model select not found');
      }
    },
    { timeout: 5000, message: 'Model select element should be present' }
  );

  // Wait for models to load
  await waitFor(
    () => {
      const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
      if (modelSelect.options.length === 0) {
        throw new Error('No model options available');
      }
    },
    { timeout: 5000, message: 'Model options should be loaded' }
  );

  // Wait for first model to be auto-selected
  await waitFor(
    () => {
      const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
      const expectedValue = modelSelect.options[0].value;
      if (modelSelect.value !== expectedValue) {
        throw new Error(`Expected model "${expectedValue}" to be selected, but got "${modelSelect.value}"`);
      }
    },
    { timeout: 5000, message: 'First model should be auto-selected' }
  );
});
```

### 3. Update documentation in [`frontend.md`](memory/frontend.md:24)

Remove the known issue note about model not being auto-selected.

## Files to Modify

1. [`frontend/src/components/MessageInput.svelte`](frontend/src/components/MessageInput.svelte) - Add reactive auto-selection
2. [`frontend/src/tests/e2e/chat.test.ts`](frontend/src/tests/e2e/chat.test.ts) - Update test with split assertions
3. [`memory/frontend.md`](memory/frontend.md) - Remove known issue note

## Testing

- Run e2e test: `cd frontend && npm run test:e2e`
- Verify new chat auto-selects first model
- Verify existing chat with saved model uses that model (reactive statement won't override non-null selectedModel)
- Verify existing chat without saved model falls back to first available
