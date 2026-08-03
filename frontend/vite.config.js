import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { aliases } from './aliases.ts';

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
  },
  resolve: {
    alias: aliases,
    conditions: ['browser'],
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/tests/setup.ts'],
  },
});
