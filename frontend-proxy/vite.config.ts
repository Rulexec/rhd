import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { proxyLogsApiPlugin, resolveLogsDir } from './src/server/index.js';

// Fail fast: throws when VITE_PROXY_LOGS_PATH is unset — vite exits with this message.
const logsDir = resolveLogsDir();

export default defineConfig({
  plugins: [svelte(), proxyLogsApiPlugin(logsDir)],
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
