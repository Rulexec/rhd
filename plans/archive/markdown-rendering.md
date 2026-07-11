# Markdown Rendering for Assistant Responses

## Overview

Add markdown rendering support for assistant responses (including thinking content) in the chat UI. Markdown rendering will be enabled by default, with a per-message toggle switch in the top-right corner of each assistant message to switch between markdown and raw text display.

## Requirements

1. **Markdown rendering enabled by default** for assistant messages
2. **Per-message toggle switch** in the top-right corner of each assistant message to enable/disable markdown for that specific message
3. **Applies to both**:
   - Regular message content (`message.content`)
   - Thinking content (`message.thinkingContent`)
4. **User messages** remain plain text (no markdown rendering)
5. **System messages** remain plain text (already displayed in `<pre>` tags)
6. **No persistence** — toggle state is per-session only
7. **Security**: Use DOMPurify to sanitize all rendered HTML

## Implementation Plan

### 1. Install Dependencies

Add `marked` for markdown parsing and `dompurify` for XSS protection:
```bash
cd frontend && npm install marked dompurify
cd frontend && npm install -D @types/marked @types/dompurify
```

### 2. Create Markdown Renderer Utility

**File**: `frontend/src/lib/markdown.ts` (new file)

Create a utility function that:
- Uses `marked` to parse markdown to HTML
- Uses `DOMPurify` to sanitize output and prevent XSS
- Handles edge cases (empty content, code blocks, etc.)

```typescript
import { marked } from 'marked';
import DOMPurify from 'dompurify';

// Configure marked for safe rendering
marked.setOptions({
  breaks: true, // Convert \n to <br>
  gfm: true, // GitHub Flavored Markdown
});

export function renderMarkdown(content: string): string {
  if (!content) return '';
  const rawHtml = marked.parse(content) as string;
  return DOMPurify.sanitize(rawHtml);
}
```

### 3. Update Message.svelte Component

**File**: `frontend/src/components/Message.svelte`

Changes:
1. Import `renderMarkdown` utility
2. Add local `markdownEnabled` state (per-message, defaults to `true`)
3. Add reactive statement to render markdown when enabled
4. Update template to use `{@html}` for markdown content
5. Add toggle button in top-right corner of assistant messages
6. Keep raw text display when markdown is disabled

```svelte
<script lang="ts">
  import { renderMarkdown } from '../lib/markdown';
  
  // ... existing code ...
  
  let markdownEnabled = true; // Per-message toggle, enabled by default
  
  $: isAssistantMessage = message.role === 'assistant';
  $: renderedContent = markdownEnabled && isAssistantMessage
    ? renderMarkdown(message.content)
    : null;
  $: renderedThinking = markdownEnabled && hasThinkingContent
    ? renderMarkdown(message.thinkingContent)
    : null;
  
  function toggleMarkdown() {
    markdownEnabled = !markdownEnabled;
  }
</script>

<!-- Toggle button for assistant messages -->
{#if isAssistantMessage && !editing}
  <button
    class="markdown-toggle-btn"
    on:click={toggleMarkdown}
    title={markdownEnabled ? 'Show raw text' : 'Show markdown'}
  >
    {markdownEnabled ? 'MD' : 'Raw'}
  </button>
{/if}

<!-- Content rendering -->
{#if renderedContent}
  <div class="content markdown-body">
    {@html renderedContent}
    {#if isStreamingMessage}
      <span class="streaming-dots"></span>
    {/if}
  </div>
{:else}
  <div class="content">
    {message.content}
    {#if isStreamingMessage}
      <span class="streaming-dots"></span>
    {/if}
  </div>
{/if}
```

Similar changes for thinking content — use `{@html renderedThinking}` when markdown is enabled, otherwise show raw `<pre>` content.

### 4. Add CSS Styles

**File**: `frontend/src/components/Message.svelte` (in `<style>` section)

Add styles for the toggle button and markdown-rendered content:

```css
/* Toggle button */
.markdown-toggle-btn {
  position: absolute;
  top: var(--spacing-xs);
  right: var(--spacing-xs);
  background: none;
  border: 1px solid var(--color-border);
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.2s;
  font-family: monospace;
}

.message.assistant:hover .markdown-toggle-btn {
  opacity: 1;
}

.markdown-toggle-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: var(--color-text);
}

/* Adjust edit button position when markdown toggle is present */
.message.assistant .edit-btn {
  right: calc(var(--spacing-xs) + 40px);
}

/* Markdown body styles */
.markdown-body {
  white-space: normal;
}

.markdown-body :global(h1),
.markdown-body :global(h2),
.markdown-body :global(h3),
.markdown-body :global(h4),
.markdown-body :global(h5),
.markdown-body :global(h6) {
  margin-top: 1em;
  margin-bottom: 0.5em;
  font-weight: 600;
}

.markdown-body :global(code) {
  background: var(--color-bg-secondary, #f6f8fa);
  padding: 0.2em 0.4em;
  border-radius: 3px;
  font-size: 0.9em;
  font-family: monospace;
}

.markdown-body :global(pre) {
  background: var(--color-bg-secondary, #f6f8fa);
  padding: 1em;
  border-radius: 6px;
  overflow-x: auto;
}

.markdown-body :global(pre code) {
  background: none;
  padding: 0;
}

.markdown-body :global(ul),
.markdown-body :global(ol) {
  padding-left: 2em;
  margin: 0.5em 0;
}

.markdown-body :global(blockquote) {
  border-left: 4px solid var(--color-border, #ddd);
  padding-left: 1em;
  margin: 0.5em 0;
  color: var(--color-text-secondary, #666);
}

.markdown-body :global(a) {
  color: var(--color-primary, #0066cc);
  text-decoration: none;
}

.markdown-body :global(a:hover) {
  text-decoration: underline;
}

.markdown-body :global(table) {
  border-collapse: collapse;
  margin: 0.5em 0;
}

.markdown-body :global(th),
.markdown-body :global(td) {
  border: 1px solid var(--color-border, #ddd);
  padding: 0.5em;
}

.markdown-body :global(th) {
  background: var(--color-bg-secondary, #f6f8fa);
  font-weight: 600;
}

/* Markdown thinking content */
.thinking-content.markdown-body {
  font-style: italic;
  color: var(--color-text-secondary, #666);
}
```

## Files to Modify

1. `frontend/package.json` — Add `marked` and `dompurify` dependencies
2. `frontend/src/lib/markdown.ts` — New file for markdown rendering utility with DOMPurify sanitization
3. `frontend/src/components/Message.svelte` — Add per-message markdown toggle and rendering

## Testing Considerations

1. **Unit tests** for markdown rendering utility (verify DOMPurify sanitization works)
2. **UI tests** for per-message toggle functionality
3. Test edge cases:
   - Empty messages
   - Messages with only whitespace
   - Code blocks with special characters
   - Streaming messages (markdown should update as content arrives)
   - Thinking content rendering
   - Toggle during streaming
   - XSS attempts in content (verify DOMPurify blocks them)

## Security

- **DOMPurify** sanitizes all HTML output from `marked` before rendering
- Strips `<script>` tags, event handlers, and other XSS vectors
- `DOMPurify.sanitize()` is called on every render, ensuring safety even with streaming content

## Future Enhancements

1. **Syntax highlighting** for code blocks (using highlight.js or Prism)
2. **Math rendering** (KaTeX or MathJax)
3. **Mermaid diagram support**
