# System Message Collapser Implementation Plan

## Overview
Add a collapsible wrapper for chat messages with `system` role in the frontend. System messages should be collapsed by default to reduce visual clutter.

## Current State
- [`Message.svelte`](frontend/src/lib/components/Message.svelte) already has a collapsible "reasoning" section pattern
- The component handles different roles (user, assistant, system) via `getRoleBadgeClass()`
- Messages are rendered in [`ChatView.svelte`](frontend/src/lib/components/ChatView.svelte) using the Message component

## Implementation Approach

### 1. Add Collapsible State for System Messages
Add a new state variable to track whether the system message content is expanded:
```typescript
let systemMessageExpanded: boolean = $state(false);
```

### 2. Add Computed Property
Add a derived property to check if the message is a system role:
```typescript
let isSystemRole: boolean = $derived(message.role === 'system');
```

### 3. Add Toggle Function
Add a function to toggle the system message expansion:
```typescript
function toggleSystemMessage(): void {
  systemMessageExpanded = !systemMessageExpanded;
}
```

### 4. Modify Template Structure
Wrap the message content in a collapsible container when role is system:

```svelte
{#if isSystemRole}
  <div class="system-message-section">
    <button class="system-message-toggle" onclick={toggleSystemMessage}>
      <span class="toggle-icon">{systemMessageExpanded ? '▼' : '▶'}</span>
      <span>System Message</span>
    </button>
    <div class="system-message-content" class:expanded={systemMessageExpanded}>
      <!-- existing message content -->
    </div>
  </div>
{:else}
  <!-- existing message content for non-system roles -->
{/if}
```

### 5. Add CSS Styling
Follow the existing reasoning section pattern:

```css
.system-message-section {
  margin-bottom: var(--spacing-sm);
}

.system-message-toggle {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-xs) 0;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  width: 100%;
  text-align: left;
}

.system-message-toggle:hover {
  color: var(--color-text);
}

.toggle-icon {
  font-size: var(--font-size-xs);
}

.system-message-content {
  max-height: 0;
  overflow: hidden;
  transition: max-height var(--transition-normal);
}

.system-message-content.expanded {
  max-height: none;
}
```

## Files to Modify
- [`frontend/src/lib/components/Message.svelte`](frontend/src/lib/components/Message.svelte)

## Testing Considerations
- Verify system messages are collapsed by default
- Verify clicking the toggle expands/collapses the content
- Verify non-system messages (user, assistant) are not affected
- Verify the collapser works correctly with streaming content
- Verify markdown toggle still works within collapsed system messages

## Visual Design
- Follow the existing reasoning section pattern for consistency
- Use the same arrow icons (▼/▶) for expand/collapse indication
- Maintain the system role badge styling (warning color)
- Ensure smooth transition animation
