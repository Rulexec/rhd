# Plan: Add `@` Alias for `src` Folder and Convert Relative Imports

## Overview

Add a `@` path alias pointing to `frontend/src/` across Vite, TypeScript, and Storybook configs. Then convert all relative imports (`../`, `./`) to absolute imports using `@/`. Finally, simplify the fragile `mockWsPlugin` in `.storybook/main.ts` to match `@/lib/ws` directly.

## Scope

- **Config files**: `vite.config.js`, `tsconfig.json`, `.storybook/main.ts`
- **Source files**: ~50 `.ts` files and ~16 `.svelte` files in `frontend/src/`
- **Total import statements to update**: ~66 relative imports

## Steps

### 1. Configure `@` alias in `frontend/vite.config.js`

Add `resolve.alias` mapping `@` to `src/`:

```js
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
    conditions: ['browser'],
  },
  // ...
});
```

### 2. Configure `@` alias in `frontend/tsconfig.json`

Add `baseUrl` and `paths` to `compilerOptions`:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  }
}
```

### 3. Configure `@` alias in `frontend/.storybook/main.ts`

Add `resolve.alias` in `viteFinal` and simplify `mockWsPlugin`:

```ts
import { resolve } from 'path';
import { fileURLToPath, URL } from 'node:url';

function mockWsPlugin(): Plugin {
  return {
    name: 'mock-ws-plugin',
    resolveId(source) {
      if (source === '@/lib/ws') {
        return resolve(process.cwd(), 'src/stories/mockWs.ts');
      }
      return null;
    },
  };
}

// In viteFinal:
viteFinal: async (viteConfig) => {
  viteConfig.plugins = viteConfig.plugins || [];
  viteConfig.plugins.push(mockWsPlugin());
  viteConfig.resolve = viteConfig.resolve || {};
  viteConfig.resolve.alias = {
    ...viteConfig.resolve.alias,
    '@': fileURLToPath(new URL('../src', import.meta.url)),
  };
  return viteConfig;
},
```

### 4. Update parent-directory relative imports (`../`) to `@/` absolute paths

Convert only imports that use `../` (parent directory references) to `@/` absolute paths. **Do NOT convert same-directory imports (`./`)** — leave those as-is.

Examples:
- `'../lib/chatStores'` → `'@/lib/chatStores'` ✓
- `'../../components/ChatView.svelte'` → `'@/components/ChatView.svelte'` ✓
- `'./stores'` → `'./stores'` (unchanged) ✗
- `'./Message.svelte'` → `'./Message.svelte'` (unchanged) ✗

Files to update:
- `frontend/src/lib/**/*.ts` (e.g., `'../chatStores'` → `'@/lib/chatStores'`)
- `frontend/src/components/**/*.svelte` (e.g., `'../lib/chatStores'` → `'@/lib/chatStores'`)
- `frontend/src/tests/**/*.ts` (e.g., `'../lib/actions'` → `'@/lib/actions'`)
- `frontend/src/stories/**/*.ts` (e.g., `'../../components/ChatView.svelte'` → `'@/components/ChatView.svelte'`)
- `frontend/src/App.svelte` and `frontend/src/main.ts` (if they use `../`)

### 5. Update `mockWs.ts` import

Change `from '../lib/types/ws'` to `from '@/lib/types/ws'`.

### 6. Verify

- Run `cd frontend && npm run check` (svelte-check) to verify no type errors
- Run `cd frontend && npm run test:unit` to verify no runtime errors

## Files Modified

| File | Change |
|------|--------|
| `frontend/vite.config.js` | Add `resolve.alias` with `@` → `src` |
| `frontend/tsconfig.json` | Add `baseUrl` and `paths` |
| `frontend/.storybook/main.ts` | Add alias in `viteFinal`, simplify `mockWsPlugin` |
| `frontend/src/**/*.ts` (~50 files) | Convert relative imports to `@/` |
| `frontend/src/**/*.svelte` (~16 files) | Convert relative imports to `@/` |
