/// <reference types="vitest/config" />
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The explorer is a static build and nothing else. Whatever serves it — the
// desktop app's explorer host, or `coffret-explorer-dev` in development — is
// the one browser entrance there is, and it alone forwards `/api` to the
// server with the server's key. So this configuration reads no key, defines no
// proxy and serves no `/api`: a dev server of its own would be a second
// entrance holding a credential, and that is the one thing there must not be
// (`make check` refuses one here).

/**
 * `@coffret/api` read from its source, for a development build only.
 *
 * The package's entry point is what `tsc -b` emitted, which a build in watch
 * mode would otherwise read once and never again: an edit to a route's types
 * or calls would not reach the explorer until the package was built again. In
 * development mode — `make web`'s build in watch mode — the package is one
 * more set of modules the build watches. A production build still goes through
 * the emitted package, which is what ships, and the cases keep resolving it
 * the way a build does.
 */
const apiSource = fileURLToPath(new URL('../../gateway/api/src/index.ts', import.meta.url));

export default defineConfig(({ mode }) => ({
  plugins: [react()],
  resolve: {
    alias: mode === 'development' ? { '@coffret/api': apiSource } : {},
  },
  // A DOM for the cases that drive a hook across renders or a handler with the
  // event a browser would hand it, which markup rendered to a string cannot
  // do. Here and in no other package: nothing below the app has a DOM to
  // speak of, and those packages keep Vitest's default.
  test: {
    environment: 'jsdom',
  },
}));
