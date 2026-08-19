import { argon2id } from 'hash-wasm';

import { AEAD_KEY_LENGTH } from '../internal/aead.js';
import { U32_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * The Argon2id cost the Passphrase is stretched at on one device (KD-5).
 *
 * The parameters are device-local policy rather than a format constant: they are
 * recorded in the stored form that used them, so raising them later re-derives
 * the protection key and rewrites only that device's stored Master Key — no
 * Storage Object changes at all (KD-6).
 */
export interface Argon2Params {
  /** Memory cost in KiB. */
  memoryKib: number;
  /** Number of passes over that memory. */
  iterations: number;
  /** How many lanes the passes are spread across. */
  parallelism: number;
}

/**
 * The values a device starts with.
 *
 * Taken from the OWASP Password Storage Cheat Sheet's Argon2id band —
 * m=19456 KiB (19 MiB), t=2, p=1, one of the several (memory, iterations) pairs
 * it lists as equivalent. A device may raise them at any time under KD-6; the
 * stored form records what it used.
 */
export const INITIAL_ARGON2_PARAMS: Argon2Params = {
  memoryKib: 19_456,
  iterations: 2,
  parallelism: 1,
};

/** Insists that a cost is one Argon2id accepts and a `u32` field can record. */
export function requireArgon2Params(params: Argon2Params): Argon2Params {
  const { memoryKib, iterations, parallelism } = params;
  const wholeU32 = (value: number): boolean =>
    Number.isInteger(value) && value >= 1 && value <= U32_MAX;
  if (!wholeU32(memoryKib) || !wholeU32(iterations) || !wholeU32(parallelism)) {
    fail(
      'invalid_argon2_params',
      `invalid Argon2id parameters: m=${memoryKib}, t=${iterations}, p=${parallelism}`,
    );
  }
  // Argon2 needs at least 8 KiB per lane; below that the parameters name no
  // derivation at all.
  if (memoryKib < 8 * parallelism) {
    fail(
      'invalid_argon2_params',
      `invalid Argon2id parameters: m=${memoryKib} is below 8 KiB per lane at p=${parallelism}`,
    );
  }
  return params;
}

/**
 * Stretches a Passphrase into the key that protects a stored Master Key.
 *
 * KD-9 records the memory cost, the iterations, the parallelism, and the salt,
 * but no Argon2 version — so the version is an agreement implementations keep
 * out of band: two builds that pick different ones derive different keys from
 * the same recorded parameters, and a form written by either never unlocks
 * under the other. The agreed version is 0x13 (v1.3), which is the only one
 * `hash-wasm` implements.
 */
export async function deriveProtectionKey(
  params: Argon2Params,
  passphrase: Uint8Array,
  salt: Uint8Array,
): Promise<Uint8Array> {
  requireArgon2Params(params);
  try {
    return await argon2id({
      password: passphrase,
      salt,
      memorySize: params.memoryKib,
      iterations: params.iterations,
      parallelism: params.parallelism,
      hashLength: AEAD_KEY_LENGTH,
      outputType: 'binary',
    });
  } catch (cause) {
    fail('passphrase_derivation_failed', 'could not derive the protection key', { cause });
  }
}
