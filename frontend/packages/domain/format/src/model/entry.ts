import type { ContainerId } from './containerId.js';

/** Length of an Entry's content hash in bytes. */
export const CONTENT_HASH_LENGTH = 32;

/** Points at the Entry that derived data — a thumbnail, a transcode — came from. */
export interface DerivedFrom {
  /** The Container holding the parent Entry. */
  containerId: ContainerId;
  /** The parent Entry's path. */
  path: string;
}

/**
 * What a Container's entry table records about one Entry (FM-9).
 *
 * `offset` and `size` place the Entry against the Container's plaintext stream,
 * which is what lets a reader range-read a single Entry out of a Pack as a step
 * in fetching its Container — the fetch unit stays the whole Container.
 */
export interface EntryMetadata {
  /** The Library position this Entry occupies. */
  path: string;
  /** Byte offset of this Entry's plaintext in the Container's plaintext stream. */
  offset: bigint;
  /** Length of this Entry's plaintext in bytes. */
  size: bigint;
  /**
   * The file's modification time, as whole seconds from the Unix epoch.
   *
   * Negative values are legal and mean "before 1970": a file can carry any
   * timestamp its filesystem allows.
   */
  mtimeSeconds: bigint;
  /** BLAKE3-256 of this Entry's plaintext. */
  hash: Uint8Array;
  /** Set when this Entry holds data derived from another Entry. */
  derivedFrom?: DerivedFrom;
  /** The media type of the content, when known. */
  mime?: string;
}

/**
 * One Entry handed to the encoder.
 *
 * The encoder derives `offset`, `size`, and `hash` itself from the position and
 * content given here, so those three cannot disagree with the bytes actually
 * stored.
 */
export interface EntrySource {
  /** The Library position this Entry occupies. */
  path: string;
  /** The file's modification time, as whole seconds from the Unix epoch. */
  mtimeSeconds: bigint;
  /** The Entry's plaintext. */
  content: Uint8Array;
  /** Set when this Entry holds data derived from another Entry. */
  derivedFrom?: DerivedFrom;
  /** The media type of the content, when known. */
  mime?: string;
}

/** One Entry recovered from a Container. */
export interface DecodedEntry {
  /** What the entry table recorded about this Entry. */
  metadata: EntryMetadata;
  /** The Entry's plaintext, verified against `metadata.hash`. */
  content: Uint8Array;
}
