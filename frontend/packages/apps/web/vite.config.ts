/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The coffret server the dev server proxies to; COFFRET_PORT targets a
// non-default backend.
const backendPort = process.env.COFFRET_PORT ?? '8787';

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': `http://127.0.0.1:${backendPort}`,
    },
  },
  test: {
    environment: 'node',
  },
});
