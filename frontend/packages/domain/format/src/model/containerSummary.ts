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
 * is the copy a record travels with.
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
   * Where the provider keeps this Container, when the writer knew.
   *
   * A device that replayed a Journal record has never seen the object and holds
   * none: the name follows from the ID alone (FM-3). A device that uploaded or
   * fetched it keeps the handle, which spares a store that mints identifiers a
   * listing.
   */
  objectRef?: string;
}
