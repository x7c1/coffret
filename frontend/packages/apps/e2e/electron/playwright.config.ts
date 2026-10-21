// How the desktop app's journey is run.
//
// Apart from the browser journeys in `journeys/`, and by a script of its own
// (`scripts/e2e-electron.sh`, `make e2e-electron`): those are walked in
// Chromium against the development host, and this one starts the desktop app
// itself, unpackaged, through Playwright's Electron driver. It needs Docker, a
// Library on MinIO, the Node-API addon and the app's built code, which the
// script makes first and hands over through the environment.

import { defineConfig } from '@playwright/test';

import { fromEnvironment } from './environment';

const setting = fromEnvironment();

export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.ts',

  // One app, one Library, one journey in order: nothing here runs twice or at
  // once, for the reasons the browser journeys give.
  fullyParallel: false,
  workers: 1,
  retries: 0,

  reporter: [['list']],
  outputDir: setting.artifacts,

  // The journey waits out the idle interval, a whole minute at the least, on
  // top of a drop reaching MinIO and being committed.
  timeout: 360_000,
  expect: { timeout: 30_000 },
});
