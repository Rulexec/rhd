<script lang="ts">
  import commonStyles from '../styles/common.module.css';

  interface Props {
    currentTags: string[];
    suggestions: string[];
    onAddTag: (tag: string) => void;
  }

  let { currentTags, suggestions, onAddTag }: Props = $props();

  let isOpen = $state(false);
  let inputValue = $state('');
  let dropdownRef: HTMLDivElement | null = $state(null);

  // Filter suggestions: exclude already-applied tags, filter by input
  let filteredSuggestions = $derived(
    suggestions
      .filter(tag => !currentTags.includes(tag))
      .filter(tag => tag.toLowerCase().includes(inputValue.toLowerCase()))
      .slice(0, 10) // Limit to 10 suggestions
  );

  // Check if input value is a new tag (not in suggestions)
  let isNewTag = $derived(
    inputValue.trim() !== '' &&
    !suggestions.some(tag => tag.toLowerCase() === inputValue.trim().toLowerCase()) &&
    !currentTags.some(tag => tag.toLowerCase() === inputValue.trim().toLowerCase())
  );

  function toggleDropdown() {
    isOpen = !isOpen;
    if (isOpen) {
      inputValue = '';
      // Focus input after dropdown opens
      setTimeout(() => {
        const input = dropdownRef?.querySelector('input');
        input?.focus();
      }, 0);
    }
  }

  function closeDropdown() {
    isOpen = false;
    inputValue = '';
  }

  function addTag(tag: string) {
    const trimmed = tag.trim();
    if (trimmed && !currentTags.includes(trimmed)) {
      onAddTag(trimmed);
      closeDropdown();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeDropdown();
    } else if (event.key === 'Enter' && inputValue.trim()) {
      event.preventDefault();
      // If there's a matching suggestion, use it; otherwise create new tag
      if (filteredSuggestions.length > 0) {
        const firstSuggestion = filteredSuggestions[0];
        if (firstSuggestion) {
          addTag(firstSuggestion);
        }
      } else if (isNewTag) {
        addTag(inputValue.trim());
      }
    }
  }

  // Close dropdown when clicking outside
  function handleClickOutside(event: MouseEvent) {
    if (dropdownRef && !dropdownRef.contains(event.target as Node)) {
      closeDropdown();
    }
  }

  // Add/remove click outside listener when dropdown opens/closes
  $effect(() => {
    if (isOpen) {
      document.addEventListener('click', handleClickOutside);
      return () => {
        document.removeEventListener('click', handleClickOutside);
      };
    }
  });
</script>

<div class="tag-input-container" bind:this={dropdownRef}>
  <div class="current-tags">
    {#each currentTags as tag (tag)}
      <span class={commonStyles.tag}>{tag}</span>
    {/each}
    <button
      class="add-tag-btn"
      onclick={toggleDropdown}
      title="Add tag"
      aria-label="Add tag"
    >
      +
    </button>
  </div>

  {#if isOpen}
    <div class="tag-dropdown">
      <input
        type="text"
        class="tag-search-input"
        placeholder="Search or create tag..."
        bind:value={inputValue}
        onkeydown={handleKeydown}
      />
      <div class="tag-suggestions">
        {#each filteredSuggestions as suggestion (suggestion)}
          <button
            class="tag-suggestion-item"
            onclick={() => addTag(suggestion)}
          >
            {suggestion}
          </button>
        {/each}
        {#if isNewTag && inputValue.trim()}
          <button
            class="tag-suggestion-item create-new"
            onclick={() => addTag(inputValue.trim())}
          >
            Create "{inputValue.trim()}"
          </button>
        {/if}
        {#if filteredSuggestions.length === 0 && !isNewTag && inputValue.trim() === ''}
          <div class="tag-suggestions-empty">
            No tags available
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .tag-input-container {
    position: relative;
    display: inline-block;
  }

  .current-tags {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    flex-wrap: wrap;
  }

  .add-tag-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-lg);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .add-tag-btn:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
    background: var(--color-primary-light);
  }

  .tag-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    margin-top: var(--spacing-xs);
    min-width: 200px;
    max-width: 300px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    z-index: 100;
    overflow: hidden;
  }

  .tag-search-input {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: transparent;
    font-size: var(--font-size-sm);
    outline: none;
  }

  .tag-search-input::placeholder {
    color: var(--color-text-muted);
  }

  .tag-suggestions {
    max-height: 200px;
    overflow-y: auto;
  }

  .tag-suggestion-item {
    display: block;
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    background: transparent;
    text-align: left;
    font-size: var(--font-size-sm);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .tag-suggestion-item:hover {
    background: var(--color-bg-secondary);
  }

  .tag-suggestion-item.create-new {
    color: var(--color-primary);
    font-style: italic;
  }

  .tag-suggestions-empty {
    padding: var(--spacing-md);
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
