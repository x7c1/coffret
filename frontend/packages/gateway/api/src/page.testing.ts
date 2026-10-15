// Not part of the package: excluded from its build by `*.testing.ts`, and run
// before every case by `vitest.config.ts`.
//
// What every case runs in: a page at the explorer host's own origin, holding a
// browser token, which is what every request this package makes is made from.
// A case about a page holding none lets it go itself.

import { beforeEach, vi } from 'vitest';

import { holdToken } from './credential';

/** The origin the cases' page is served from. */
export const ORIGIN = 'http://127.0.0.1:5173';

/** The token the cases' page holds. */
export const TOKEN = 'the-token-this-tab-holds';

beforeEach(() => {
  vi.stubGlobal('location', new URL(`${ORIGIN}/`));
  holdToken(TOKEN);
});
