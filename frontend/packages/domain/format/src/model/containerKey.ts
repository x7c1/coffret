import { drawBytes } from '../internal/entropy.js';
import { SecretBytes } from './secretBytes.js';

/** Length of a Container Key in bytes. */
export const CONTAINER_KEY_LENGTH = 32;

/**
 * The 256-bit key that encrypts exactly one Container (KD-2).
 *
 * Each Container Key is drawn independently and is never derived from the
 * Master Key, which is what lets one Container be replaced or discarded without
 * re-keying any other.
 */
export class ContainerKey extends SecretBytes {
  private constructor(bytes: Uint8Array) {
    super(bytes, CONTAINER_KEY_LENGTH, 'ContainerKey');
  }

  /** Takes 32 raw bytes. */
  static fromBytes(bytes: Uint8Array): ContainerKey {
    return new ContainerKey(bytes);
  }
}

/**
 * Draws a fresh Container Key from the platform CSPRNG.
 *
 * No derivation path exists from one Container's key to another's: independent
 * keys keep a future single-Container sharing path open (KD-2).
 */
export function generateContainerKey(): ContainerKey {
  return ContainerKey.fromBytes(drawBytes(CONTAINER_KEY_LENGTH));
}
