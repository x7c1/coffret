// How the explorer is measured against the large Library.
//
// Apart from the journeys, and by a script of its own (`scripts/large.sh
// measure`, `make large-measure`): this asserts nothing about the explorer and
// times it instead, against a Library of a hundred thousand files that the
// script seeded, served by the server and the development host the script
// started. It needs Docker and a browser, so `make check` never runs it.

import { defineConfig } from '@playwright/test';

import { fromEnvironment } from './environment';

const setting = fromEnvironment();

export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.ts',

  // One tab, one Library, one measurement in order: a second at once would
  // time the two against each other.
  fullyParallel: false,
  workers: 1,
  retries: 0,

  reporter: [['list']],
  outputDir: setting.artifacts,

  // Opening the tree to twenty thousand rows is hundreds of listings, opened
  // one folder at a time, each waited on.
  timeout: 3_600_000,
  expect: { timeout: 60_000 },

  use: {
    baseURL: setting.webUrl,
    // The journeys' window, so the rows on show are the ones a person sees.
    viewport: { width: 1440, height: 900 },
    headless: true,
    launchOptions: setting.chromium === undefined ? {} : { executablePath: setting.chromium },
  },
});
