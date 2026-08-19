import { bytesEqual, fromHex, takeExactly, toHex } from '../internal/bytes.js';
import { drawBytes } from '../internal/entropy.js';

/** Length of a Container ID in bytes. */
export const CONTAINER_ID_LENGTH = 16;

/** Length of a Container ID in hex characters. */
export const CONTAINER_ID_HEX_LENGTH = CONTAINER_ID_LENGTH * 2;

/** Extension every coffret Storage Object name ends with. */
export const STORAGE_EXTENSION = '.cfrt';

/**
 * The 128-bit identifier a Container carries for its whole life (FM-3).
 *
 * The identifier is drawn from a CSPRNG and takes no input from the content it
 * names, which is what lets the object name derived from it say nothing about
 * what the object holds.
 */
export class ContainerId {
  readonly #bytes: Uint8Array;

  private constructor(bytes: Uint8Array) {
    this.#bytes = takeExactly(bytes, CONTAINER_ID_LENGTH, 'a Container ID');
  }

  /** Takes 16 raw bytes. */
  static fromBytes(bytes: Uint8Array): ContainerId {
    return new ContainerId(bytes);
  }

  /** Parses the 32-lowercase-hex-character spelling. */
  static fromHex(hex: string): ContainerId {
    return new ContainerId(fromHex(hex, CONTAINER_ID_HEX_LENGTH));
  }

  /** The raw 16 bytes, as a copy the caller owns. */
  bytes(): Uint8Array {
    return Uint8Array.from(this.#bytes);
  }

  /** The 32-lowercase-hex-character spelling. */
  toHex(): string {
    return toHex(this.#bytes);
  }

  /**
   * The name this Container is stored under: the ID as 32 lowercase hex
   * characters followed by `.cfrt` (FM-3).
   */
  objectName(): string {
    return `${this.toHex()}${STORAGE_EXTENSION}`;
  }

  /** Whether two identifiers name the same Container. */
  equals(other: ContainerId): boolean {
    return bytesEqual(this.#bytes, other.#bytes);
  }

  toString(): string {
    return this.toHex();
  }

  toJSON(): string {
    return this.toHex();
  }
}

/** Draws a fresh Container ID from the platform CSPRNG. */
export function generateContainerId(): ContainerId {
  return ContainerId.fromBytes(drawBytes(CONTAINER_ID_LENGTH));
}
