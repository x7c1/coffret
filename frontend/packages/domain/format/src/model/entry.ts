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
 *
 * These are the values as of the moment the Container was written, which is why
 * the meta section spells the ones a rename could move `original_path`,
 * `original_mtime`, and `original_btime` (FM-9). A Container is immutable, so
 * nothing rewrites them; the Journal and its checkpoint carry the current
 * spelling, which is why a record and a Snapshot say `path`, `mtime`, and
 * `btime` for the same values (FM-15, FM-16). One interface serves both because
 * the values are the same values — only the map key differs.
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
  /**
   * When the file came into being, as whole seconds from the Unix epoch, where
   * the platform that wrote the Container reported one.
   *
   * Absent means no birth time was ever captured — never "created at the
   * epoch". Not every platform and filesystem keeps one, and a birth time
   * cannot be recovered once the original file is gone, so this is written when
   * the Container is and never stamped onto a fetched file.
   */
  btimeSeconds?: bigint;
  /** BLAKE3-256 of this Entry's plaintext. */
  hash: Uint8Array;
  /** Set when this Entry holds data derived from another Entry. */
  derivedFrom?: DerivedFrom;
  /**
   * The media type of the content, when known.
   *
   * A guess made when the Container was written, and a hint to a reader rather
   * than a verdict: what may be opened is decided elsewhere (FM-9).
   */
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
  /** The file's birth time, where the platform reported one. */
  btimeSeconds?: bigint;
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
