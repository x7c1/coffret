/**
 * Key Envelopes: a Container Key wrapped under the container-wrap purpose key.
 *
 * An envelope is `nonce(24) ‖ ciphertext(32) ‖ tag(16)` — 72 bytes — with the
 * 16-byte Container ID as associated data (FM-14), so an envelope presented for
 * a different Container fails to unwrap and envelopes cannot be swapped between
 * Containers. Envelopes live in the Keyring and never in a Container itself,
 * which is what lets a Master Key rotation rewrite every envelope while leaving
 * Containers byte-for-byte unchanged.
 */

import { open, seal } from './internal/aead.js';
import { concatBytes, takeExactly } from './internal/bytes.js';
import { NONCE_LENGTH, randomNonce } from './internal/nonce.js';
import { CONTAINER_KEY_LENGTH, ContainerKey } from './model/containerKey.js';
import { KeyEnvelope } from './model/keyEnvelope.js';
import { purposeKeyBytes, type PurposeKey } from './purposeKey.js';
import type { ContainerId } from './model/containerId.js';

/** Wraps a Container Key into the envelope the Keyring stores for it. */
export function wrapContainerKey(
  key: PurposeKey,
  containerId: ContainerId,
  containerKey: ContainerKey,
  /** The nonce to wrap under; drawn from the CSPRNG when left out. */
  nonce: Uint8Array = randomNonce(),
): KeyEnvelope {
  const keyBytes = purposeKeyBytes(key, 'container-wrap');
  const message = seal(
    keyBytes,
    takeExactly(nonce, NONCE_LENGTH, 'a nonce'),
    containerId.bytes(),
    containerKey.bytes(),
  );
  return KeyEnvelope.fromBytes(concatBytes(nonce, message));
}

/**
 * Opens the envelope a Keyring holds for one Container.
 *
 * The Container ID goes in as associated data rather than being read out of the
 * envelope, so an envelope that belongs to another Container fails
 * authentication instead of yielding the wrong key.
 */
export function unwrapContainerKey(
  key: PurposeKey,
  containerId: ContainerId,
  envelope: KeyEnvelope,
): ContainerKey {
  const keyBytes = purposeKeyBytes(key, 'container-wrap');
  const bytes = envelope.bytes();
  const plaintext = open(
    keyBytes,
    bytes.subarray(0, NONCE_LENGTH),
    containerId.bytes(),
    bytes.subarray(NONCE_LENGTH),
  );
  return ContainerKey.fromBytes(takeExactly(plaintext, CONTAINER_KEY_LENGTH, 'a Container Key'));
}
