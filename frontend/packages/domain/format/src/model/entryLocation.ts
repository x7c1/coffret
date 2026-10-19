import type { ContainerId } from './containerId.js';
import type { EntryMetadata } from './entry.js';

/**
 * Where the Entry currently at one Entry Path lives.
 *
 * This is the mapping an Index exists for: an Entry Path answered with the
 * Container that holds it and the Entry's place inside that Container, so that
 * opening a file needs no lookup on Storage and no Container is opened to find
 * out what another one holds (spec: RV-5).
 *
 * At every committed Library state one Entry Path identifies at most one current
 * Entry, so a location is the whole answer for a path (spec: EP-5).
 */
export interface EntryLocation {
  /** The Container holding this Entry. */
  containerId: ContainerId;
  /**
   * The Entry's 0-based position in that Container's entry table: with
   * `containerId`, the name for this Entry that a rename leaves unchanged
   * (spec: FM-9).
   */
  entryNumber: number;
  /** What that Container's entry table records about it (spec: FM-9). */
  entry: EntryMetadata;
  /**
   * When the content this Entry holds was deposited into the Library, as whole
   * seconds from the Unix epoch (spec: FM-15, FM-16).
   *
   * The catalog's own value and none of the Container's: no meta section
   * carries it, and a rename or a rewrite that carries this content into another
   * Entry carries the time with it. A separate axis from the file's own
   * `mtimeSeconds` and `btimeSeconds`.
   */
  depositedAtSeconds: bigint;
}
