import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';

import { AEAD_KEY_LENGTH } from './internal/aead.js';
import { asciiBytes } from './internal/bytes.js';
import { fail } from './errors.js';
import type { ControlObjectKind } from './model/kinds.js';
import type { MasterKey } from './model/masterKey.js';

/**
 * What a key derived from the Master Key is allowed to encrypt (KD-4).
 *
 * The Master Key is never an AEAD key itself: every use passes through HKDF
 * with the purpose's info string, so a key derived for one purpose is useless
 * for another and adding a purpose is adding an info string.
 */
export type Purpose =
  | 'container-wrap'
  | 'control/journal'
  | 'control/keyring'
  | 'control/index-snapshot'
  // The only purpose so far whose key protects device-local state rather than a
  // Storage Object: the OAuth token cache a device keeps for a Storage
  // provider. It is in the registry because the registry is the
  // specification's, not any one implementation's.
  | 'token-cache';

/**
 * The v1 purpose registry: the info string each purpose derives under.
 *
 * These strings are format constants — changing one would orphan every object
 * already written under it.
 */
export const PURPOSE_INFO: Readonly<Record<Purpose, string>> = {
  'container-wrap': 'coffret/v1/container-wrap',
  'control/journal': 'coffret/v1/control/journal',
  'control/keyring': 'coffret/v1/control/keyring',
  'control/index-snapshot': 'coffret/v1/control/index-snapshot',
  'token-cache': 'coffret/v1/token-cache',
};

/** Every purpose the v1 registry lists, for callers that must cover them all. */
export const PURPOSES: readonly Purpose[] = [
  'container-wrap',
  'control/journal',
  'control/keyring',
  'control/index-snapshot',
  'token-cache',
];

/** The purpose that encrypts payloads of the given control-object kind. */
export function purposeOfControlObject(kind: ControlObjectKind): Purpose {
  switch (kind) {
    case 'journal':
      return 'control/journal';
    case 'keyring':
      return 'control/keyring';
    case 'index-snapshot':
      return 'control/index-snapshot';
  }
}

/** Length of a purpose key in bytes. */
export const PURPOSE_KEY_LENGTH = AEAD_KEY_LENGTH;

/**
 * The derived bytes of every purpose key, kept beside the keys rather than in
 * them: nothing outside this module can reach a key's material, whatever it
 * does with the object.
 */
const DERIVED_BYTES = new WeakMap<PurposeKey, Uint8Array>();

/**
 * A 256-bit key derived from the Master Key for exactly one purpose.
 *
 * Derivation is HKDF-SHA-256 with the Master Key as input keying material, a
 * zero-length salt, the purpose's info string, and a 32-byte output (KD-3). The
 * Master Key itself never encrypts anything, so a purpose that leaks costs the
 * Library that purpose and nothing else.
 *
 * A key carries the purpose it was derived for, and every operation that takes
 * one checks that purpose before using it — separate keys only separate
 * anything if nothing crosses them over.
 */
export class PurposeKey {
  /** What this key is allowed to encrypt. */
  readonly purpose: Purpose;

  private constructor(purpose: Purpose, bytes: Uint8Array) {
    this.purpose = purpose;
    DERIVED_BYTES.set(this, bytes);
  }

  /** Derives the key for one purpose from the Master Key. */
  static derive(masterKey: MasterKey, purpose: Purpose): PurposeKey {
    const bytes = hkdf(
      sha256,
      masterKey.bytes(),
      new Uint8Array(0),
      asciiBytes(PURPOSE_INFO[purpose]),
      PURPOSE_KEY_LENGTH,
    );
    return new PurposeKey(purpose, bytes);
  }

  toString(): string {
    return `PurposeKey(${PURPOSE_INFO[this.purpose]}, <redacted>)`;
  }

  toJSON(): string {
    return this.toString();
  }
}

/**
 * The raw key bytes, once the caller's purpose matches this key's.
 *
 * Deliberately not part of the package's public surface: it exists so the
 * encoders and decoders in this package can reach the bytes, and nothing else
 * can.
 */
export function purposeKeyBytes(key: PurposeKey, expected: Purpose): Uint8Array {
  if (key.purpose !== expected) {
    fail(
      'wrong_purpose_key',
      `this message needs the ${PURPOSE_INFO[expected]} key, not the ${PURPOSE_INFO[key.purpose]} key`,
    );
  }
  const bytes = DERIVED_BYTES.get(key);
  if (bytes === undefined) {
    fail('wrong_purpose_key', 'this key was not derived by this package');
  }
  return bytes;
}
