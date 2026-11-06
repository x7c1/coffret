import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    // The helpers' own cases alone. The journeys and the measurement are
    // `*.spec.ts`, which Playwright runs against a Library on MinIO
    // (scripts/e2e-it.sh, scripts/large.sh), and which vitest would otherwise
    // take for tests of its own.
    include: ['journeys/**/*.test.ts', 'scale/**/*.test.ts'],
  },
});
