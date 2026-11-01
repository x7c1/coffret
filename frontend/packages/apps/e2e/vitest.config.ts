import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    // The helpers' own cases alone. The journeys are `*.spec.ts`, which
    // Playwright runs against a Library on MinIO (scripts/e2e-it.sh), and
    // which vitest would otherwise take for tests of its own.
    include: ['journeys/**/*.test.ts'],
  },
});
