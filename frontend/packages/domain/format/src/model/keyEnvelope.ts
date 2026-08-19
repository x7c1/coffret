import { bytesEqual, takeExactly } from '../internal/bytes.js';

/** Length of a Key Envelope in bytes. */
export const KEY_ENVELOPE_LENGTH = 72;

/**
 * One Container Key, encrypted so that only the Master Key can open it.
 *
 * The envelope is 72 bytes — `nonce(24) ‖ ciphertext(32) ‖ tag(16)` — and is
 * bound to the Container it belongs to, so an envelope presented for a
 * different Container fails to unwrap (FM-14).
 *
 * The bytes are ciphertext, so unlike the keys themselves this type is ordinary
 * data: it compares and prints like any other identifier.
 */
export class KeyEnvelope {
  readonly #bytes: Uint8Array;

  private constructor(bytes: Uint8Array) {
    this.#bytes = takeExactly(bytes, KEY_ENVELOPE_LENGTH, 'a Key Envelope');
  }

  /** Takes 72 raw bytes. */
  static fromBytes(bytes: Uint8Array): KeyEnvelope {
    return new KeyEnvelope(bytes);
  }

  /** The raw 72 bytes, as a copy the caller owns. */
  bytes(): Uint8Array {
    return Uint8Array.from(this.#bytes);
  }

  /** Whether two envelopes hold the same bytes. */
  equals(other: KeyEnvelope): boolean {
    return bytesEqual(this.#bytes, other.#bytes);
  }
}
