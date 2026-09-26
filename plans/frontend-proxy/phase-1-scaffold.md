# Phase 1: Scaffold `frontend-proxy` Application

## Overview

Create a runnable, type-checked, testable empty Svelte application at `frontend-proxy/` (repo
root, sibling of `frontend/`) using the **same technology stack and conventions** as the main
frontend: Svelte 5 with runes, TypeScript strict mode, Vite 6, Vitest 3, svelte-check, MobX, Zod.
The dev server listens on **port 5174** with `strictPort: true`.

This phase delivers no feature logic — only the project skeleton, a placeholder root component,
and mise tasks so every later phase has uniform verification commands.

**Scope in:** project files, placeholder App shell, mise task wiring.
**Scope out:** Vite middleware and `VITE_PROXY_LOGS_PATH` handling (Phase 2), stores (Phase 3),
real UI (Phases 4–5), docs (Phase 6).

**Milestone context:** this is the viewer for `rhd_ai_proxy`'s chat-logging SQLite database
(`chats.sqlite3`, tables `chats` / `prefix_hashes` / `requests` / `raw`). Full background in
`plans/frontend-proxy-logs-viewer-grand-plan.md`.

## Files to Create

### 1. `frontend-proxy/package.json`

Copy the dependency set from `frontend/package.json` verbatim; change `name` and keep all
scripts. `better-sqlite3` is **not** added here (Phase 2 adds it when the middleware lands).

```json
{
  "name": "rhd-proxy-frontend",
  "private": true,
  "version": "0.0.1",
  "type": "module",
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "type-check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run"
  },
  "dependencies": {
    "marked": "^15.0.0",
    "mobx": "^6.13.0",
    "zod": "^3.23.0"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5.0.0",
    "@testing-library/svelte": "^5.2.0",
    "@types/node": "^26.2.0",
    "@typescript/native": "npm:typescript@^7.0.2",
    "jsdom": "^26.0.0",
    "svelte": "^5.0.0",
    "svelte-check": "^4.7.6",
    "typescript": "^6.0.3",
    "vite": "^6.0.0",
    "vitest": "^3.0.0"
  }
}
```

### 2. `frontend-proxy/vite.config.ts`

Copy of `frontend/vite.config.ts` with the port changed. The Phase 2 middleware plugin gets wired
in here later; nothing else changes.

```typescript
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5174,
    strictPort: true
  },
  css: {
    modules: {
      localsConvention: 'camelCase'
    }
  }
});
```

### 3. `frontend-proxy/svelte.config.ts`

Exact copy of `frontend/svelte.config.ts` (runes mode is required by all later component work):

```typescript
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    runes: true
  }
};
```

### 4. `frontend-proxy/tsconfig.json`

Exact copy of `frontend/tsconfig.json` — strict flags, `types: ["node"]` (needed later for
`process.env` in the vite config / server module), `moduleResolution: "bundler"`,
`allowImportingTsExtensions`, `noEmit`. Keep the same `include` (`src/**/*.ts`,
`src/**/*.svelte`) and `exclude` (`node_modules`, `dist`) — Phase 2's server code lives under
`src/server/`, so it stays covered by svelte-check automatically.

### 5. `frontend-proxy/vitest.config.ts`

Copy of `frontend/vitest.config.ts`. Default environment is jsdom (component tests); Phase 2's
server tests opt into Node per-file with a `@vitest-environment node` docblock, so no config
change is needed for them.

```typescript
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ['browser']
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts']
  }
});
```

**Note:** the `resolve.conditions: ['browser']` entry only affects how *module resolution* picks
browser vs node builds of dependencies for the test transform — it does not prevent server-side
tests from running under the Node environment (better-sqlite3 is a native addon loaded directly,
not a conditional-exports package).

### 6. `frontend-proxy/index.html`

Copy of `frontend/index.html` with a different title and stylesheet path stays identical:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>RHD Proxy Logs</title>
  <link rel="stylesheet" href="/src/global.css" />
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/src/main.ts"></script>
</body>
</html>
```

### 7. `frontend-proxy/src/main.ts`

Exact copy of `frontend/src/main.ts` (Svelte 5 `mount` API):

```typescript
import { mount } from 'svelte';
import App from './App.svelte';

const app = mount(App, {
  target: document.getElementById('app')!
});

export default app;
```

### 8. `frontend-proxy/src/App.svelte`

Placeholder shell — header only. Phase 4 replaces the body with the two-pane layout and store
wiring; keeping it trivial now means this phase has no dependency on store/API code.

```svelte
<script lang="ts">
  // Placeholder shell; real layout + store wiring land in Phase 4.
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Proxy Logs</h1>
  </header>

  <main class="app-main">
    <p class="placeholder">Viewer not wired up yet — see Phase 4.</p>
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
    padding: var(--spacing-lg);
  }

  .placeholder {
    color: var(--color-text-muted);
  }
</style>
```

### 9. `frontend-proxy/src/global.css`

Copy `frontend/src/global.css` verbatim (design tokens, error/loading/focus/scrollbar styles).
Later phases reference the same CSS custom properties (`--color-*`, `--spacing-*`,
`--font-size-*`, `--radius-*`, `--transition-*`).

### 10. `frontend-proxy/src/lib/styles/common.module.css`

Copy `frontend/src/lib/styles/common.module.css` verbatim. Later components reuse its classes
(`btn`, `btn-primary`, `btn-sm`, `list`, `list-item`, `active`, `tag`, `text-muted`,
`text-error`, `truncate`, …) exactly like `frontend/src/lib/components/ChatList.svelte` does via
`import commonStyles from '../styles/common.module.css'`.

### 11. `frontend-proxy/src/vite-env.d.ts`

Exact copy of `frontend/src/vite-env.d.ts`:

```typescript
/// <reference types="svelte" />
/// <reference types="vite/client" />
```

### 12. `frontend-proxy/.nvmrc`

Copy of `frontend/.nvmrc` (single line: `v24.13.0`).

### 13. `frontend-proxy/.gitignore`

Copy of `frontend/.gitignore`:

```
node_modules
dist
.DS_Store
*.log
```

## Files to Modify

### 14. `mise.toml` (repo root)

Add three standalone tasks after the existing `[tasks]` entries, and wire the check task into the
existing `check` group:

```toml
[tasks.dev-frontend-proxy]
run = ["npm --prefix frontend-proxy run dev"]

[tasks.check-frontend-proxy]
run = ["npm --prefix frontend-proxy run check"]

[tasks.test-frontend-proxy]
run = ["npm --prefix frontend-proxy run test"]
```

Change the existing `check` task to also run the new check (keep `check-cargo` first):

```toml
[tasks.check]
depends = ["check-cargo", "check-frontend-proxy"]
```

**Do not** touch `test-all` / `test-cargo` — cargo tests are unaffected by this app, and wiring
npm tests into the cargo pipeline is out of scope (the main `frontend/` isn't wired either;
changing that is a separate decision).

## Tests

This phase ships no application code to test — its "test" is that the toolchain works:

1. `cd frontend-proxy && npm install` — installs cleanly on Node v24.13.0 (`.nvmrc`).
2. `npm run check` — svelte-check passes with zero errors (this is the real gate that configs are
   correct and `App.svelte` compiles under runes mode).
3. `npm run test` — vitest runs and reports "no test files found" **without config errors**
   (exit code may be non-zero for no tests; if it errors, add `passWithNoTests: true` to the
   `test` block in `vitest.config.ts` — prefer this over adding a dummy test).
4. `npm run dev` — server starts on `http://localhost:5174`, serves the placeholder page;
   starting a second instance fails loudly (strictPort).
5. `mise run check` from repo root — both `check-cargo` and `check-frontend-proxy` pass.

## Implementation Notes

1. **Why duplicate instead of share (AD-1):** `frontend-proxy` is a debug tool with a different
   lifecycle and data source than the product frontend. Small copies (configs, css, two util
   files in Phase 3) are cheaper than introducing a shared npm workspace the repo doesn't have.
   If a third consumer ever appears, extract then.
2. **strictPort (AD-10):** silent port hopping would break the documented URL and any bookmarked
   deep links; failing loudly matches `frontend/`'s behavior.
3. **`@types/node` is kept** even though no Node API is used in this phase — Phase 2 needs it
   (`process.env`, `path`, `fs`, `node:http` in tests) and `tsconfig.json` pins `"types":
   ["node"]`, which would break svelte-check if the package were absent.
4. **Node version pin:** better-sqlite3 (Phase 2) ships prebuilt binaries per Node ABI; the
   `.nvmrc` copy keeps both frontends on the same version so one `nvm use` covers the repo.
5. **File-size limits:** all files here are small; the project convention (< 500 lines, prefer
   < 400) applies to the new app as it grows.

## Dependencies

- **Depends on:** nothing — first phase.
- **Blocks:** Phase 2 (needs the scaffold, vitest config, mise tasks), Phase 3+ (build on top).
- **Parallel with:** nothing meaningful; everything else builds on this scaffold.

## Verification Summary

```sh
cd frontend-proxy && npm install
npm --prefix frontend-proxy run check   # or: mise run check-frontend-proxy
npm --prefix frontend-proxy run test    # or: mise run test-frontend-proxy
npm --prefix frontend-proxy run dev     # http://localhost:5174 placeholder
mise run check                          # cargo + frontend-proxy check
```
