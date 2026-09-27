import fs from 'node:fs';
import path from 'node:path';
import Database from 'better-sqlite3';

/** File name of the logging database inside the configured directory (matches rhd_ai_proxy). */
export const DB_FILE_NAME = 'chats.sqlite3';

/**
 * Logging schema version this viewer understands (mirror of `SCHEMA_VERSION` in
 * packages/rhd_ai_proxy/src/logging/schema.rs — the viewer must not depend on the
 * Rust crate). Databases stamped with any other version are rejected at open.
 */
export const EXPECTED_LOGGING_SCHEMA_VERSION = 2;

/** How long queries wait on the database lock before failing (concurrent proxy writers). */
const BUSY_TIMEOUT_MS = 5000;

/**
 * Handle to the opened logging database.
 *
 * Opened in normal (read-write) mode on purpose: SQLite cannot open a WAL-mode
 * database read-only when no writer is attached (the -shm file needs write
 * access for recovery), and the proxy may well be offline while the viewer is
 * used. Read-only behavior is enforced by the query layer (queries.ts contains
 * only SELECTs). WAL lets the proxy write concurrently while we read.
 */
export interface LogsDb {
  raw: Database.Database;
}

export function openLogsDb(logsDir: string): LogsDb {
  const dbFile = path.join(logsDir, DB_FILE_NAME);
  if (!fs.existsSync(dbFile)) {
    throw new Error(
      `logging database not found at ${dbFile}. ` +
        `Start rhd_ai_proxy with proxy.logging enabled (logging.path = ${logsDir}) ` +
        `so the database is created, then restart this dev server.`
    );
  }
  const raw = new Database(dbFile);
  raw.pragma(`busy_timeout = ${BUSY_TIMEOUT_MS}`);

  const version = raw.pragma('user_version', { simple: true });
  if (version !== EXPECTED_LOGGING_SCHEMA_VERSION) {
    raw.close();
    throw new Error(
      `logging database at ${dbFile} has schema version ${version}, but this viewer ` +
        `understands version ${EXPECTED_LOGGING_SCHEMA_VERSION}. The database predates or ` +
        `postdates the current rhd_ai_proxy logging schema — delete or move the file and run ` +
        `the proxy once with logging enabled to start a fresh one.`
    );
  }
  return { raw };
}

export function closeLogsDb(db: LogsDb): void {
  db.raw.close();
}
