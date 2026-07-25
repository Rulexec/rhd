/**
 * Async sleep function for testing
 */
export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Wait for assertions to pass within a timeout.
 * Repeatedly calls the assertion function until it passes or times out.
 *
 * @param assertions - Function containing expect() assertions
 * @param options - timeout (ms) and interval (ms) between retries
 * @throws The last error if timeout is reached
 *
 * @example
 * await waitFor(() => {
 *   expect(get(isPaused)).toBe(true);
 *   expect(get(isStreaming)).toBe(false);
 * });
 */
export async function waitFor(
  assertions: () => void,
  options: { timeout?: number; interval?: number } = {}
): Promise<void> {
  const { timeout = 5000, interval = 50 } = options;
  const startTime = Date.now();
  let lastError: Error | null = null;
  
  while (Date.now() - startTime < timeout) {
    try {
      assertions();
      return;
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      await sleep(interval);
    }
  }
  
  throw lastError ?? new Error(`waitFor timed out after ${timeout}ms`);
}
