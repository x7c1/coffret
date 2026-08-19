import { drawBytes } from '../internal/entropy.js';
import { SecretBytes } from './secretBytes.js';

/** Length of a Master Key in bytes. */
export const MASTER_KEY_LENGTH = 32;

/**
 * The 256-bit key every purpose key in a Library is derived from.
 *
 * It is drawn from a CSPRNG and never from the Passphrase or any other
 * user-chosen input (KD-1), so the strength of the ciphertext on Storage never
 * depends on passphrase quality. Each Master Key epoch draws its own.
 */
export class MasterKey extends SecretBytes {
  private constructor(bytes: Uint8Array) {
    super(bytes, MASTER_KEY_LENGTH, 'MasterKey');
  }

  /** Takes 32 raw bytes. */
  static fromBytes(bytes: Uint8Array): MasterKey {
    return new MasterKey(bytes);
  }
}

/** Draws a fresh Master Key from the platform CSPRNG. */
export function generateMasterKey(): MasterKey {
  return MasterKey.fromBytes(drawBytes(MASTER_KEY_LENGTH));
}
