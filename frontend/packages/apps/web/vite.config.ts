/// <reference types="vitest/config" />
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { defineConfig, type ProxyOptions } from 'vite';
import react from '@vitejs/plugin-react';

// The coffret server the dev server proxies to; COFFRET_PORT targets a
// non-default backend.
const backendPort = process.env.COFFRET_PORT ?? '8787';
const backend = `http://127.0.0.1:${backendPort}`;

// The header that server admits a caller by. It refuses every request that does
// not carry the key it drew as it started, which is what keeps a page on some
// other site from reaching a Library through the browser this explorer runs in.
const KEY_HEADER = 'x-coffret-key';

/**
 * The file the running server wrote its key into, or `null` where nothing said
 * which Library is being served.
 *
 * Named outright by COFFRET_SERVER_KEY_FILE, or worked out from the Library's
 * name the way the server itself lays a Library directory out: under
 * COFFRET_STATE_DIR, or the state directory the platform names, then
 * `libraries/<name>/server-key`.
 */
function keyFile(): string | null {
  const named = process.env.COFFRET_SERVER_KEY_FILE;
  if (named !== undefined && named !== '') {
    return named;
  }
  const library = process.env.COFFRET_LIBRARY;
  if (library === undefined || library === '') {
    return null;
  }
  return path.join(stateRoot(), 'libraries', library, 'server-key');
}

/** Coffret's own directory under the state directory the platform names. */
function stateRoot(): string {
  const state = process.env.COFFRET_STATE_DIR;
  if (state !== undefined && state !== '') {
    return state;
  }
  const xdg = process.env.XDG_STATE_HOME;
  if (xdg !== undefined && xdg !== '') {
    return path.join(xdg, 'coffret');
  }
  return path.join(process.env.HOME ?? '', '.local', 'state', 'coffret');
}

/**
 * The key that file holds right now, or `null` where it cannot be read.
 *
 * Read per request rather than once at startup, and that is not laziness: a
 * server draws a new key every time it starts, and this proxy outlives more than
 * one of them — it is aimed at a port before the first server is up, and the
 * journeys that kill the server and start it again leave a key behind that the
 * next process has already replaced.
 */
function currentKey(): string | null {
  const file = keyFile();
  if (file === null) {
    return null;
  }
  try {
    return readFileSync(file, 'utf8').trim();
  } catch {
    // No server has started yet, or this is not the account that owns the
    // Library. Either way there is no key to send, and the server says so in
    // terms a person can act on — which is better than anything invented here.
    return null;
  }
}

/**
 * `/api`, forwarded to the coffret server with the key it admits callers by.
 *
 * The key is attached here rather than in the page, and that is the point of
 * proxying at all: this runs on the device and can read the file, the browser
 * cannot, and so the key never reaches anything a page could read it out of.
 */
const api: ProxyOptions = {
  target: backend,
  // The server refuses a `Host` naming anywhere but where it is, because a
  // hostname somebody else's DNS pointed at that socket arrives carrying that
  // name. Without this the forwarded request would still carry *this* server's
  // address, which is not that one either.
  changeOrigin: true,
  configure(proxy) {
    // Which key file this proxy will read, said once in the terminal it was
    // started from. The server prints the same path as it starts, so the two
    // halves can be read side by side — and a refusal on the screen cannot say
    // this, because it is the server's own answer and the half that is aimed
    // wrong is this one.
    const file = keyFile();
    if (file === null) {
      console.warn(
        '[coffret] nothing says which Library is being served, so /api carries no key ' +
          'and the server refuses it: set COFFRET_LIBRARY (`make web LIBRARY=<name>` does) ' +
          'or COFFRET_SERVER_KEY_FILE.',
      );
    } else {
      console.log(`[coffret] /api goes to ${backend}, with the key at ${file}`);
    }

    proxy.on('proxyReq', (proxyReq, request) => {
      // Removed before it is set, so that nothing a caller of this proxy put
      // under this name is what gets forwarded.
      proxyReq.removeHeader(KEY_HEADER);
      const key = currentKey();
      if (key !== null) {
        proxyReq.setHeader(KEY_HEADER, key);
      }

      // A request the page this proxy serves made carries that page's origin,
      // which is this server rather than the one behind it. Put right so that
      // the server sees its own; every other origin is left exactly as it
      // arrived, which is what lets the server refuse a page on another site.
      const origin = request.headers.origin;
      if (origin !== undefined && origin === `http://${request.headers.host}`) {
        proxyReq.setHeader('origin', backend);
      }
    });
  },
};

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { '/api': api },
  },
  // Stated rather than inherited from the dev server's: `vite preview` is how
  // the built explorer is served to the journeys, and the key has to reach the
  // server from there too.
  preview: {
    proxy: { '/api': api },
  },
  test: {
    environment: 'node',
  },
});
