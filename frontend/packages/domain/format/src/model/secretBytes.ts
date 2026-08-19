import { takeExactly } from '../internal/bytes.js';

/**
 * A fixed-length secret that never prints itself.
 *
 * Key material reaches a log line through a formatter more easily than through
 * deliberate code, so every key type keeps its bytes in a private field and
 * spells itself `<redacted>` in a string context and in JSON. The bytes are
 * handed out only through [`bytes`], and as a copy, so a holder cannot reach
 * back into the key and change it.
 *
 * Zeroization is not implemented: JavaScript offers no way to erase a value the
 * runtime may have copied. Keeping the copies few and deliberate is what this
 * type can do.
 */
export abstract class SecretBytes {
  readonly #bytes: Uint8Array;
  readonly #label: string;

  protected constructor(bytes: Uint8Array, byteLength: number, label: string) {
    this.#bytes = takeExactly(bytes, byteLength, `a ${label}`);
    this.#label = label;
  }

  /** The raw bytes, as a copy the caller owns. */
  bytes(): Uint8Array {
    return Uint8Array.from(this.#bytes);
  }

  toString(): string {
    return `${this.#label}(<redacted>)`;
  }

  toJSON(): string {
    return this.toString();
  }
}
