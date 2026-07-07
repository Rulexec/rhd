# New Chat Dialog Modal Plan

## Objective
Replace `prompt('Enter chat title:')` in [`ChatList.svelte`](frontend/src/components/ChatList.svelte:6) with a frontend-only modal using the native HTML `<dialog>` element, centered on screen with a fade overlay.

## Current Implementation
- File: [`frontend/src/components/ChatList.svelte`](frontend/src/components/ChatList.svelte)
- Line 6: `const title = prompt('Enter chat title:');`
- Uses browser's native `prompt()` which blocks UI and looks inconsistent

## Target Implementation

### 1. Add State Variables
```typescript
let showDialog = false;
let chatTitle = '';
let dialogEl: HTMLDialogElement;
```

### 2. Replace `handleCreateChat()` Logic
- Remove `prompt()` call
- Set `showDialog = true` and call `dialogEl.showModal()`

### 3. Add Dialog HTML Structure
```svelte
<dialog bind:this={dialogEl} class="chat-dialog">
  <form method="dialog" on:submit={handleSubmit}>
    <h3>New Chat</h3>
    <input 
      type="text" 
      bind:value={chatTitle} 
      placeholder="Enter chat title"
      autofocus
    />
    <div class="dialog-actions">
      <button type="button" on:click={handleCancel}>Cancel</button>
      <button type="submit" disabled={!chatTitle.trim()}>Create</button>
    </div>
  </form>
</dialog>
```

### 4. Add Event Handlers
- `handleSubmit()`: Create chat with trimmed title, close dialog, reset state
- `handleCancel()`: Close dialog, reset state
- Listen for dialog `close` event to sync `showDialog` state

### 5. Add CSS Styles
```css
.chat-dialog {
  border: none;
  border-radius: 8px;
  padding: var(--spacing-xl);
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  max-width: 400px;
  width: 90%;
}

.chat-dialog::backdrop {
  background: rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(2px);
}

.chat-dialog h3 {
  margin: 0 0 var(--spacing-m) 0;
  font-size: 18px;
  font-weight: 600;
}

.chat-dialog input {
  width: 100%;
  padding: var(--spacing-s) var(--spacing-m);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  font-size: 14px;
  margin-bottom: var(--spacing-l);
}

.chat-dialog input:focus {
  outline: none;
  border-color: var(--color-primary);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-s);
}

.dialog-actions button {
  padding: var(--spacing-s) var(--spacing-m);
  border-radius: 4px;
  font-size: 14px;
  cursor: pointer;
}

.dialog-actions button[type="button"] {
  background: none;
  border: 1px solid var(--color-border);
  color: var(--color-text);
}

.dialog-actions button[type="submit"] {
  background: var(--color-primary);
  color: white;
  border: none;
}

.dialog-actions button[type="submit"]:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
```

## Implementation Steps

1. **Add state variables** to `<script>` section
2. **Modify `handleCreateChat()`** to open dialog instead of calling `prompt()`
3. **Add `handleSubmit()` and `handleCancel()` functions**
4. **Add `<dialog>` element** with form, input, and action buttons
5. **Add CSS styles** for dialog, backdrop, input, and buttons
6. **Test**: Click "+ New Chat", verify modal appears centered with fade overlay, input is focused, can type title, Create button works, Cancel closes dialog, Escape key closes dialog

## Browser Compatibility
Native `<dialog>` element supported in all modern browsers (Chrome 37+, Firefox 98+, Safari 15.4+). No polyfill needed.

## Accessibility
- Dialog has implicit `role="dialog"` and `aria-modal="true"`
- Form uses `method="dialog"` for proper form submission
- Input has `autofocus` for immediate typing
- Escape key closes dialog natively
- Create button disabled when input empty
