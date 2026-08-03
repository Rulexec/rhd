# Plan: Add Storybook to Frontend

## Overview

Add Storybook to the Svelte 5 frontend for component development and documentation. Create a single story in `frontend/src/stories/pause-abort/` that renders `ChatView`.

## Current State

- **Framework**: Svelte 5.28.2 with runes (`$state`, `$derived`, `$mcpStatuses`)
- **Build tool**: Vite 6.3.5
- **TypeScript**: 5.7.2
- **Testing**: Vitest with happy-dom

## Implementation Steps

### Step 1: Install Storybook Dependencies

Install Storybook for Svelte with Vite builder:

```bash
cd frontend
npm install --save-dev \
  storybook@^8.6.0 \
  @storybook/svelte@^8.6.0 \
  @storybook/svelte-vite@^8.6.0 \
  @storybook/addon-essentials@^8.6.0 \
  @storybook/addon-interactions@^8.6.0 \
  @storybook/addon-links@^8.6.0 \
  @storybook/blocks@^8.6.0 \
  @storybook/test@^8.6.0
```

**Note**: Storybook 8.x has full Svelte 5 support.

### Step 2: Initialize Storybook Configuration

Create `.storybook/` directory with configuration files:

**`.storybook/main.ts`**:
```typescript
import type { StorybookConfig } from '@storybook/svelte-vite';

const config: StorybookConfig = {
  stories: ['../src/**/*.mdx', '../src/**/*.stories.@(js|ts|svelte)'],
  addons: [
    '@storybook/addon-links',
    '@storybook/addon-essentials',
    '@storybook/addon-interactions',
  ],
  framework: {
    name: '@storybook/svelte-vite',
    options: {},
  },
};

export default config;
```

**`.storybook/preview.ts`**:
```typescript
import type { Preview } from '@storybook/svelte';

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
};

export default preview;
```

### Step 3: Add Storybook Scripts to package.json

Add the following scripts to `frontend/package.json`:

```json
{
  "scripts": {
    "storybook": "storybook dev -p 6006",
    "build-storybook": "storybook build"
  }
}
```

### Step 4: Create ChatView Story

Create the story file at `frontend/src/stories/pause-abort/ChatView.stories.ts`:

```typescript
import type { Meta, StoryObj } from '@storybook/svelte';
import ChatView from '../../components/ChatView.svelte';

// Mock the stores that ChatView depends on
import { currentChat, chats } from '../../lib/chatStores';
import { chatProjects, mcpStatuses } from '../../lib/projectStores';

// Set up mock data
currentChat.set({
  id: 'chat-1',
  title: 'Test Chat',
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  active_model: 'gpt-4',
  project_names: [],
});

chats.set([
  {
    id: 'chat-1',
    title: 'Test Chat',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    active_model: 'gpt-4',
    project_names: [],
  },
]);

chatProjects.set([]);
mcpStatuses.set([]);

const meta = {
  title: 'Pause-Abort/ChatView',
  component: ChatView,
  tags: ['autodocs'],
  parameters: {
    layout: 'fullscreen',
  },
} satisfies Meta<ChatView>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {},
};

export const WithProjects: Story = {
  args: {},
  beforeEach: () => {
    chatProjects.set([
      { name: 'project-alpha', path: '/path/to/alpha' },
      { name: 'project-beta', path: '/path/to/beta' },
    ]);
    mcpStatuses.set([
      { projectName: 'project-alpha', status: 'connected', serverName: 'server1' },
      { projectName: 'project-beta', status: 'connecting', serverName: 'server2' },
    ]);
  },
};

export const EmptyChat: Story = {
  args: {},
  beforeEach: () => {
    currentChat.set({
      id: 'chat-empty',
      title: 'Empty Chat',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      active_model: null,
      project_names: [],
    });
  },
};
```

### Step 5: Handle Store Dependencies

The `ChatView` component uses Svelte stores that need to be mocked. The story will:

1. Import the actual store instances
2. Set initial values before rendering
3. Use `beforeEach` hooks to reset state between stories

**Alternative approach** (if store mocking causes issues):

Create a wrapper component `ChatViewStoryWrapper.svelte` that provides mock context:

```svelte
<script lang="ts">
  import ChatView from '../components/ChatView.svelte';
  import { currentChat, chats } from '../lib/chatStores';
  import { chatProjects, mcpStatuses } from '../lib/projectStores';
  
  export let mockChat = {
    id: 'test-chat',
    title: 'Test Chat',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    active_model: 'gpt-4',
    project_names: [],
  };
  
  $: currentChat.set(mockChat);
  $: chats.set([mockChat]);
  $: chatProjects.set([]);
  $: mcpStatuses.set([]);
</script>

<ChatView />
```

### Step 6: CSS Variables Setup

The component uses CSS variables defined in `frontend/src/styles/global.css`. Import this file in `.storybook/preview.ts` to reuse the existing variables:

```typescript
import '../src/styles/global.css';

const preview: Preview = {
  parameters: {
    // ... existing config
  },
};
```

This ensures Storybook uses the same CSS variables as the main application, avoiding duplication.

## File Structure

```
frontend/
├── .storybook/
│   ├── main.ts
│   └── preview.ts
├── src/
│   ├── stories/
│   │   └── pause-abort/
│   │       └── ChatView.stories.ts
│   └── ... (existing files)
└── package.json (updated with storybook scripts)
```

## Verification

1. Run `npm run storybook` to start Storybook dev server
2. Navigate to `http://localhost:6006`
3. Verify the "Pause-Abort/ChatView" story appears in the sidebar
4. Check that the component renders correctly with mock data
5. Test all story variants (Default, WithProjects, EmptyChat)

## Potential Issues & Solutions

### Issue 1: Svelte 5 Runes Compatibility
**Problem**: Storybook might not fully support Svelte 5 runes syntax.
**Solution**: Use Storybook 8.6+ which has Svelte 5 support. If issues persist, use the wrapper component approach.

### Issue 2: Store Dependencies
**Problem**: Stores might not be properly initialized in Storybook context.
**Solution**: Use `beforeEach` hooks or wrapper components to set store values before rendering.

### Issue 3: CSS Variables Missing
**Problem**: Component styles rely on CSS variables not available in Storybook.
**Solution**: Import global CSS or define variables in `.storybook/preview.css`.

### Issue 4: Child Component Dependencies
**Problem**: `ChatView` imports many child components that might have their own dependencies.
**Solution**: Ensure all child components are properly mocked or their dependencies are satisfied.

## Next Steps (Future)

After this initial setup, consider:
- Adding more stories for other components (MessageList, MessageInput, etc.)
- Setting up visual regression testing with Chromatic
- Adding interaction tests for user actions
- Creating documentation with MDX files
