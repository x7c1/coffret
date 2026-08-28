import type { ContainerId } from './containerId.js';
import type { ContainerKind } from './kinds.js';

/**
 * What the Library records about one current Container without opening it.
 *
 * It is the Container-level half of what a Journal record's additions carry
 * (CP-11) and what an Index Snapshot's `containers` lists (FM-16): the kind and
 * the ciphertext hash, kept so that neither answering "which Containers are
 * current" nor selecting `freeze` candidates has to open a Container.
 *
 * The Container's own meta section stays the authority on what it holds; this
 * is the copy replaying a record or restoring from a Snapshot leaves behind.
 */
export interface ContainerSummary {
  /** The identifier this Container carries for its whole life (FM-3). */
  id: ContainerId;
  /** Whether the Container was made one file at a time or by the pack policy (PK-15). */
  kind: ContainerKind;
  /** BLAKE3-256 of the Container's ciphertext, as its Journal record recorded it. */
  ciphertextHash: Uint8Array;
  /** Length of the Container's ciphertext in bytes. */
  ciphertextLength: bigint;
  /**
   * Storage's own identifier for this Container's object, when one is recorded.
   *
   * The value is the same whichever device reads it, and it is carried as a
   * cache so that a fetch needs no listing first (FM-15, FM-16). A Journal
   * record and an Index Snapshot both carry it, so a device holds whatever the
   * record it replayed or the Snapshot it restored from recorded; absent says
   * only that no writer recorded a reference — a name-keyed Storage, or a
   * writer that had none — and the Container is then reached by the name its ID
   * gives it (FM-3).
   *
   * It is never evidence of membership: a listing re-derives it, and a device
   * that cannot open the object it names falls back to the listing rather than
   * failing (FM-15).
   */
  objectRef?: string;
}
