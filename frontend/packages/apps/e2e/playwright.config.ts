// How the explorer's journeys are run.
//
// Never by `pnpm -r test`, and that is the reason the script is called
// `test:e2e`: this suite needs Docker, a Library on MinIO, two binaries and a
// browser, none of which a package's own test run has any business requiring.
// What starts it is `scripts/e2e-it.sh`, which builds all of that first and
// hands it over through the environment.

import { defineConfig } from '@playwright/test';

import { fromEnvironment } from './journeys/environment';

const setting = fromEnvironment();

export default defineConfig({
  testDir: 'journeys',

  // One worker, in file order, and no parallelism. The journeys are one
  // journey: they share a Library on one disk, and the order is part of what
  // they prove — the backfill journey only means anything while the album's
  // files have never been fetched.
  fullyParallel: false,
  workers: 1,

  // And no retries, for the same reason. A second attempt would run against a
  // Library the first attempt had already changed, which is a different
  // journey and would report a pass for something nobody ran.
  retries: 0,

  reporter: [['list']],
  outputDir: setting.artifacts,

  // Generous, because these wait on real work: a fetch reaches MinIO, and a
  // drop is not finished until a sync has committed a head.
  timeout: 180_000,
  expect: { timeout: 30_000 },

  use: {
    baseURL: setting.webUrl,
    // Big enough for the tree, the columns and a page of a book at a size a
    // person can judge — the pictures are the deliverable.
    viewport: { width: 1440, height: 900 },
    // What a failed run leaves to be read afterwards, beyond the checkpoint
    // pictures the journeys take for themselves.
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },

  // The explorer as it is built, served the way `make web` serves it: a static
  // bundle with `/api` proxied to the coffret server. `preview.proxy` falls
  // back to the dev server's, which is already aimed by COFFRET_PORT — so this
  // is the whole of pointing the page at the server this run started.
  webServer: {
    // Loopback and nothing else, as the server it proxies to binds: the pages
    // it serves read a Library's plaintext, and an interface anybody else is on
    // would be that plaintext offered to whoever else is on the network.
    command:
      `pnpm exec vite preview --host 127.0.0.1 --port ${setting.webPort} --strictPort`,
    cwd: setting.webPackage,
    url: setting.webUrl,
    // Nothing else may be answering there: a preview left over from another run
    // would be serving another build against another server.
    reuseExistingServer: false,
    timeout: 60_000,
    env: { COFFRET_PORT: String(setting.serverPort) },
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
