import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // The package is bytes in, bytes out: it touches no DOM, so the plain Node
    // environment is the one that proves it.
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
