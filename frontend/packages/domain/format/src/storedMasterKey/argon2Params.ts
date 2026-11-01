import { argon2id } from 'hash-wasm';

import { AEAD_KEY_LENGTH } from '../internal/aead.js';
import { U32_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * The Argon2id cost the Passphrase is stretched at on one device (spec: KD-5).
 *
 * The parameters are device-local policy rather than a format constant: they are
 * recorded in the stored form that used them, so raising them later re-derives
 * the protection key and rewrites only that device's stored Master Key — no
 * Storage Object changes at all (spec: KD-6).
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

/**
 * The most a stored form may ask for, each parameter inclusive (spec: KD-9).
 *
 * A ceiling on what a reader accepts, not a cost anything derives at: a form
 * within it unlocks under the cost it records. The recorded cost is
 * authenticated only once the derivation it asks for has run, so without a
 * ceiling a form edited to an enormous cost would exhaust memory or never finish
 * before the Passphrase is judged. A device strengthening its cost stays within
 * it (spec: KD-6).
 */
export const ARGON2_PARAMS_CEILING: Argon2Params = {
  memoryKib: 262_144,
  iterations: 8,
  parallelism: 4,
};

/** The parameters in the order of KD-9's ceiling table, named as its layout table names them. */
const CEILING_CHECKS: readonly [keyof Argon2Params, string][] = [
  ['memoryKib', 'memory cost in KiB'],
  ['iterations', 'iterations'],
  ['parallelism', 'parallelism'],
];

/**
 * Insists that no parameter of a cost exceeds {@link ARGON2_PARAMS_CEILING}
 * (spec: KD-9).
 *
 * The refusal names the first parameter over its ceiling, and nothing is retried
 * at a lower cost: a form that asks for more than a reader accepts is refused,
 * never opened under a cost it does not record.
 */
export function requireWithinCeiling(params: Argon2Params): Argon2Params {
  for (const [field, parameter] of CEILING_CHECKS) {
    const value = params[field];
    const ceiling = ARGON2_PARAMS_CEILING[field];
    if (value > ceiling) {
      fail(
        'argon2_cost_above_ceiling',
        `an Argon2id ${parameter} of ${value} is above the ${ceiling} a stored Master Key may ask for`,
      );
    }
  }
  return params;
}

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
