# Thinking Collapsible: Collapsed Preview + Smart Auto-Scroll

## Goal

Change thinking collapsible behavior in chat messages:
1. **Collapsed state**: show last 3 visual lines (rendered/wrapped, not raw `\n` splits) so user sees live streaming progress
2. **Expanded state**: auto-scroll to bottom; continue auto-scroll while user stays at bottom; stop if user scrolls up; resume if user scrolls back to bottom

## Files to Modify

- [`frontend/src/components/Message.svelte`](frontend/src/components/Message.svelte) — thinking block for saved messages
- [`frontend/src/components/StreamingMessage.svelte`](frontend/src/components/StreamingMessage.svelte) — thinking block for streaming messages

Both components have identical thinking collapsible structure (header button + `{#if thinkingExpanded}` block + `<pre>` content).

## Implementation

### 1. Collapsed Preview (last 3 visual lines)

**Approach**: Instead of hiding content entirely when collapsed, show it with a constrained `max-height` that fits exactly 3 visual lines, using `overflow: hidden`. Content naturally shows the *top* — to show the *bottom*, use a CSS trick: wrap in a flex container that pushes content down, or use `object-position` / negative margin.

**Chosen technique**: Wrap `<pre>` in a container div. When collapsed, container has `max-height: 3 * line-height` (font-size 12px × line-height 1.5 = 18px × 3 = 54px) + padding. Use `display: flex; flex-direction: column; justify-content: flex-end;` on the container so content aligns to bottom, with `overflow: hidden` clipping the top.

**CSS changes** (both files):
```css
.thinking-preview {
  max-height: calc(3 * 1.5em + 16px); /* 3 lines + padding */
  overflow: hidden;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
}

.thinking-preview .thinking-content {
  margin: 0;
}
```

**Template change**: Replace `{#if thinkingExpanded}...{/if}` with:
```svelte
{#if thinkingExpanded}
  <pre class="thinking-content" bind:this={thinkingEl} on:scroll={handleScroll}>
    {thinkingContent}
  </pre>
{:else}
  <div class="thinking-preview">
    <pre class="thinking-content">{thinkingContent}</pre>
  </div>
{/if}
```

### 2. Smart Auto-Scroll (expanded state)

**State variables**:
```ts
let thinkingEl: HTMLPreElement | null = null;
let autoScroll = true;
const SCROLL_THRESHOLD = 30; // px from bottom considered "at bottom"
```

**Scroll handler** — detect user intent:
```ts
function handleScroll() {
  if (!thinkingEl) return;
  const { scrollTop, scrollHeight, clientHeight } = thinkingEl;
  const atBottom = scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
  autoScroll = atBottom;
}
```

**Reactive auto-scroll** — on content change, scroll if user at bottom:
```ts
$: if (thinkingExpanded && thinkingEl && autoScroll) {
  thinkingEl.scrollTop = thinkingEl.scrollHeight;
}
```

**On expand** — scroll to bottom immediately and enable auto-scroll:
```ts
function toggleThinking() {
  thinkingExpanded = !thinkingExpanded;
  if (thinkingExpanded) {
    autoScroll = true;
    tick().then(() => {
      if (thinkingEl) thinkingEl.scrollTop = thinkingEl.scrollHeight;
    });
  }
}
```

Use `tick()` to wait for DOM update after expanding.

### 3. Apply to Both Components

Both `Message.svelte` and `StreamingMessage.svelte` need identical changes. In `Message.svelte`, the thinking block is inside the `{:else}` branch (non-system messages). In `StreamingMessage.svelte`, it's at the top level.

## Todo List

- [ ] Modify `Message.svelte`: add collapsed preview wrapper, auto-scroll logic, scroll handler
- [ ] Modify `StreamingMessage.svelte`: same changes
- [ ] Test: collapsed shows last 3 lines, updates during streaming
- [ ] Test: expand scrolls to bottom, auto-scrolls on new content
- [ ] Test: scroll up stops auto-scroll, scroll back to bottom resumes it
