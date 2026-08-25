import type { ContainerSummary } from './containerSummary.js';
import type { EntryLocation } from './entryLocation.js';
import type { IndexCheckpoint } from './indexCheckpoint.js';

/**
 * The whole Library-wide content of an Index, as an Index Snapshot carries it.
 *
 * A Snapshot holds the Index of the whole Library — every current Entry and its
 * Container, including Entries under subtrees the writing device does not map —
 * and no device state at all: no local root mappings, no local paths, no record
 * of which Entries a device has materialized, no spool locations, and no record
 * of which checkpoint this Index adopted (CK-7, EP-9, EP-10). That is what lets
 * two devices laid out differently restore identical content from one Snapshot.
 */
export interface SnapshotContent {
  /** The committed Library state this content stands at (CK-1, CK-2, CK-3). */
  checkpoint: IndexCheckpoint;
  /** The current Containers, ordered by Container ID. */
  containers: ContainerSummary[];
  /** Every current Entry and where it lives, ordered by Entry Path bytes (EP-3). */
  entries: EntryLocation[];
}
