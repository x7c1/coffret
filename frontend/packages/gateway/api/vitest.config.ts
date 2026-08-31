import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // The package is a wire contract: what it has to get right is the shape of
    // a response and the shape of a refusal, neither of which needs a DOM.
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
