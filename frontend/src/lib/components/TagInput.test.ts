import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import TagInput from './TagInput.svelte';

/**
 * Unit tests for TagInput component.
 */
describe('TagInput', () => {
  const mockOnAddTag = vi.fn();

  beforeEach(() => {
    mockOnAddTag.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it('should render current tags', () => {
    render(TagInput, {
      props: {
        currentTags: ['tag1', 'tag2'],
        suggestions: ['tag1', 'tag2', 'tag3'],
        onAddTag: mockOnAddTag
      }
    });

    expect(screen.getByText('tag1')).toBeTruthy();
    expect(screen.getByText('tag2')).toBeTruthy();
  });

  it('should render add button', () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    expect(addButton).toBeTruthy();
  });

  it('should open dropdown when add button is clicked', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    expect(input).toBeTruthy();
  });

  it('should filter suggestions based on input', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['apple', 'banana', 'apricot'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    await fireEvent.input(input, { target: { value: 'ap' } });

    expect(screen.getByText('apple')).toBeTruthy();
    expect(screen.getByText('apricot')).toBeTruthy();
    expect(screen.queryByText('banana')).toBeNull();
  });

  it('should exclude already-applied tags from suggestions', async () => {
    render(TagInput, {
      props: {
        currentTags: ['tag1'],
        suggestions: ['tag1', 'tag2', 'tag3'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    // tag1 should be in current tags but not in suggestions dropdown
    const currentTags = screen.getByText('tag1');
    expect(currentTags).toBeTruthy();
    
    // Check that tag1 is not in the suggestions dropdown
    const suggestions = screen.queryAllByRole('button');
    const tag1InSuggestions = suggestions.find(btn => btn.textContent === 'tag1');
    expect(tag1InSuggestions).toBeUndefined();
    
    expect(screen.getByText('tag2')).toBeTruthy();
    expect(screen.getByText('tag3')).toBeTruthy();
  });

  it('should call onAddTag when suggestion is clicked', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const tag2Button = screen.getByText('tag2');
    await fireEvent.click(tag2Button);

    expect(mockOnAddTag).toHaveBeenCalledWith('tag2');
  });

  it('should show create new option when input does not match suggestions', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    await fireEvent.input(input, { target: { value: 'new-tag' } });

    expect(screen.getByText(/create "new-tag"/i)).toBeTruthy();
  });

  it('should call onAddTag when create new is clicked', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    await fireEvent.input(input, { target: { value: 'new-tag' } });

    const createButton = screen.getByText(/create "new-tag"/i);
    await fireEvent.click(createButton);

    expect(mockOnAddTag).toHaveBeenCalledWith('new-tag');
  });

  it('should call onAddTag when Enter is pressed with new tag', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    await fireEvent.input(input, { target: { value: 'new-tag' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    expect(mockOnAddTag).toHaveBeenCalledWith('new-tag');
  });

  it('should close dropdown when Escape is pressed', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: ['tag1', 'tag2'],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    const input = screen.getByPlaceholderText(/search or create tag/i);
    expect(input).toBeTruthy();

    await fireEvent.keyDown(input, { key: 'Escape' });

    expect(screen.queryByPlaceholderText(/search or create tag/i)).toBeNull();
  });

  it('should show empty state when no suggestions and no input', async () => {
    render(TagInput, {
      props: {
        currentTags: [],
        suggestions: [],
        onAddTag: mockOnAddTag
      }
    });

    const addButton = screen.getByRole('button', { name: /add tag/i });
    await fireEvent.click(addButton);

    expect(screen.getByText(/no tags available/i)).toBeTruthy();
  });
});
