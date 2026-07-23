import { fileURLToPath } from 'node:url';

export const aliases: Record<string, string> = {
  '@': fileURLToPath(new URL('./src', import.meta.url)),
};
