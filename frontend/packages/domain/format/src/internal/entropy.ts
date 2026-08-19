/**
 * The one place this package draws randomness from.
 *
 * Every identifier, key, salt, and random nonce coffret writes comes from the
 * platform CSPRNG through here, so there is a single answer to where the
 * entropy came from and a single place a failure to get it is reported. The
 * source is Web Crypto, which both a browser and Node supply, so nothing here
 * ties the package to one of them.
 */

import { fail } from '../errors.js';

interface WebCrypto {
  getRandomValues<T extends ArrayBufferView>(array: T): T;
}

/** Draws `length` bytes from the platform CSPRNG. */
export function drawBytes(length: number): Uint8Array {
  const source = (globalThis as { crypto?: WebCrypto }).crypto;
  if (source?.getRandomValues === undefined) {
    fail('entropy_unavailable', 'no Web Crypto getRandomValues on this platform');
  }
  return source.getRandomValues(new Uint8Array(length));
}
