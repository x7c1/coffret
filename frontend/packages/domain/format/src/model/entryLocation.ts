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
  /** What that Container's entry table records about it (spec: FM-9). */
  entry: EntryMetadata;
}
