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

  // The explorer as it is built, served the way `make web` serves it: the
  // development host, `coffret-explorer-dev`, serving the build from disk with
  // `/api` forwarded to the coffret server. The host reads the key the server
  // admits callers by out of the Library's directory — on this device, so the
  // key never reaches the page — and admits the page only by a browser token,
  // which each journey gets by opening the launch file the host writes into
  // `launchDir` (see `openExplorer` in `journeys/journey.ts`). Nothing turns that off.
  //
  // The host outlives every server the journeys start, including the ones that
  // replace a killed process; it reads the server's key file per request, and writes a
  // fresh launch file — the host's generation retired, every session with it —
  // when the key in it changes.
  webServer: {
    command: [
      setting.hostBinary,
      '--library',
      setting.library,
      '--server-port',
      String(setting.serverPort),
      '--port',
      String(setting.webPort),
      '--dist',
      setting.webDist,
      '--launch-dir',
      setting.launchDir,
    ]
      .map((argument) => `'${argument.replaceAll("'", "'\\''")}'`)
      .join(' '),
    url: setting.webUrl,
    // Nothing else may be answering there: a host left over from another run
    // would be serving another build against another server.
    reuseExistingServer: false,
    timeout: 60_000,
    env: {
      COFFRET_STATE_DIR: setting.stateDir,
      COFFRET_LOG_DIR: setting.logDir,
    },
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
