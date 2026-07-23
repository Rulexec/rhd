import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { aliases } from './aliases.ts';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: aliases,
    conditions: ['browser'],
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/tests/setup.ts'],
    include: ['src/tests/e2e/**/*.test.ts'],
    testTimeout: 60000,
  },
});
