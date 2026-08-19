/**
 * Test support: naming the failure a call is expected to raise.
 *
 * Excluded from the package build — nothing here ships.
 */

import { CoffretFormatError, type CoffretErrorCode } from './errors.js';

/** Runs `body` and returns the [`CoffretErrorCode`] it failed with. */
export function errorCode(body: () => unknown): CoffretErrorCode {
  try {
    body();
  } catch (error) {
    return codeOf(error);
  }
  throw new Error('expected the call to fail, but it returned');
}

/** Awaits `body` and returns the [`CoffretErrorCode`] it failed with. */
export async function asyncErrorCode(body: () => Promise<unknown>): Promise<CoffretErrorCode> {
  try {
    await body();
  } catch (error) {
    return codeOf(error);
  }
  throw new Error('expected the call to fail, but it resolved');
}

function codeOf(error: unknown): CoffretErrorCode {
  if (error instanceof CoffretFormatError) {
    return error.code;
  }
  throw error;
}
