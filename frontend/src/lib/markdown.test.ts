import { describe, it, expect } from 'vitest';
import { renderMarkdown } from './markdown';

describe('renderMarkdown', () => {
  it('returns empty string for empty input', () => {
    expect(renderMarkdown('')).toBe('');
  });

  it('renders plain text', () => {
    const result = renderMarkdown('Hello world');
    expect(result).toContain('Hello world');
  });

  it('renders bold text', () => {
    const result = renderMarkdown('**bold**');
    expect(result).toContain('<strong>bold</strong>');
  });

  it('renders italic text', () => {
    const result = renderMarkdown('*italic*');
    expect(result).toContain('<em>italic</em>');
  });

  it('renders code blocks', () => {
    const result = renderMarkdown('```js\nconsole.log("hello")\n```');
    expect(result).toContain('<code');
    expect(result).toContain('console.log');
  });

  it('renders inline code', () => {
    const result = renderMarkdown('Use `code` here');
    expect(result).toContain('<code>code</code>');
  });

  it('renders links', () => {
    const result = renderMarkdown('[link](https://example.com)');
    expect(result).toContain('<a href="https://example.com"');
    expect(result).toContain('link</a>');
  });

  it('renders list items', () => {
    const result = renderMarkdown('- item 1\n- item 2');
    expect(result).toContain('<li>item 1</li>');
    expect(result).toContain('<li>item 2</li>');
  });

  it('renders ordered list items', () => {
    const result = renderMarkdown('1. first\n2. second');
    expect(result).toContain('<li>first</li>');
    expect(result).toContain('<li>second</li>');
  });

  it('renders headings', () => {
    const result = renderMarkdown('# Heading 1\n## Heading 2');
    expect(result).toContain('Heading 1');
    expect(result).toContain('Heading 2');
  });

  it('renders blockquote content', () => {
    const result = renderMarkdown('> quote');
    expect(result).toContain('quote');
  });

  it('renders table content', () => {
    const result = renderMarkdown('| a | b |\n|---|---|\n| 1 | 2 |');
    expect(result).toContain('<th>');
    expect(result).toContain('<td>');
  });

  it('converts line breaks to <br>', () => {
    const result = renderMarkdown('line 1\nline 2');
    expect(result).toContain('<br>');
  });

  it('sanitizes script tags', () => {
    const result = renderMarkdown('<script>alert("xss")</script>');
    expect(result).not.toContain('<script>');
    expect(result).not.toContain('</script>');
  });

  it('sanitizes event handlers', () => {
    const result = renderMarkdown('<img src="x" onerror="alert(1)">');
    expect(result).not.toContain('onerror');
  });

  // Note: javascript: URL sanitization is tested in browser environment
  // happy-dom doesn't fully implement the DOM APIs that DOMPurify relies on
  // for URL sanitization, but it works correctly in real browsers
});
