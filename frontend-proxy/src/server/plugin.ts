import type { Plugin } from 'vite';
import { openLogsDb } from './db.js';
import { createApiMiddleware } from './routes.js';

/** Environment variable pointing at the directory that contains chats.sqlite3. */
export const PROXY_LOGS_PATH_ENV = 'VITE_PROXY_LOGS_PATH';

/**
 * Resolve and validate the logging directory from the environment.
 * Throws a descriptive error when unset — called from vite.config.ts so the
 * dev server fails fast at config load.
 */
export function resolveLogsDir(env: NodeJS.ProcessEnv = process.env): string {
  const value = env[PROXY_LOGS_PATH_ENV];
  if (!value || value.trim() === '') {
    throw new Error(
      `${PROXY_LOGS_PATH_ENV} is not set. It must point at the directory that ` +
        `contains rhd_ai_proxy's chats.sqlite3 logging database — the same folder ` +
        `configured as proxy.logging.path in the proxy YAML. Example: ` +
        `${PROXY_LOGS_PATH_ENV}=./packages/rhd_ai_proxy/proxy-logs npm run dev`
    );
  }
  return value;
}

/**
 * Vite plugin exposing the read-only proxy-logs JSON API on the dev server.
 * The database is opened once when the server starts.
 */
export function proxyLogsApiPlugin(logsDir: string): Plugin {
  return {
    name: 'rhd-proxy-logs-api',
    configureServer(server) {
      const db = openLogsDb(logsDir);
      server.middlewares.use(createApiMiddleware(db));
    }
  };
}
