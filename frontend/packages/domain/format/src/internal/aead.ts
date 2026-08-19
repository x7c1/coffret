/**
 * The one AEAD construction format v1 uses.
 *
 * Every AEAD message coffret writes — a Container's meta section and chunks,
 * control-object payloads, Key Envelopes, and a device's stored Master Key — is
 * XChaCha20-Poly1305 with a 256-bit key and a 24-byte nonce, laid down as
 * `ciphertext ‖ tag(16)` (FM-1). A message that fails authentication is
 * rejected whole: this module never hands back plaintext it could not
 * authenticate.
 *
 * The cipher takes bare key bytes rather than one key type, because the keys
 * that reach it come from several places — a Container Key, one of the HKDF
 * purpose keys, a Passphrase-derived protection key — and which key belongs to
 * which message is the caller's rule to keep rather than this module's.
 */

import { xchacha20poly1305 } from '@noble/ciphers/chacha.js';

import { fail } from '../errors.js';

/** Length of a Poly1305 authentication tag in bytes. */
export const TAG_LENGTH = 16;

/** Length of an AEAD key in bytes. */
export const AEAD_KEY_LENGTH = 32;

/** Encrypts `plaintext`, returning `ciphertext ‖ tag`. */
export function seal(
  key: Uint8Array,
  nonce: Uint8Array,
  associatedData: Uint8Array,
  plaintext: Uint8Array,
): Uint8Array {
  return xchacha20poly1305(key, nonce, associatedData).encrypt(plaintext);
}

/**
 * Authenticates `message` (`ciphertext ‖ tag`) and returns its plaintext.
 *
 * The plaintext is returned only once the tag verifies, so a caller can never
 * observe bytes from an unauthenticated message.
 */
export function open(
  key: Uint8Array,
  nonce: Uint8Array,
  associatedData: Uint8Array,
  message: Uint8Array,
): Uint8Array {
  if (message.length < TAG_LENGTH) {
    fail('truncated', `an AEAD message is at least ${TAG_LENGTH} bytes, found ${message.length}`);
  }
  try {
    return xchacha20poly1305(key, nonce, associatedData).decrypt(message);
  } catch (cause) {
    fail('authentication_failed', 'message failed authentication', { cause });
  }
}
