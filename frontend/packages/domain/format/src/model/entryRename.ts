import type { ContainerId } from './containerId.js';

/**
 * The top-level component coffret reserves for derived Entries, such as
 * thumbnails. No rename gives an Entry a path there (spec: FM-15).
 */
export const DERIVED_COMPONENT = '.coffret-derived';

/** The largest entry number the format admits: entry numbers are u32 (spec: FM-9). */
export const MAX_ENTRY_NUMBER = 0xffff_ffff;

/**
 * One element of a Journal record's `renames`: the Entry a commit gives a new
 * current Entry Path (spec: FM-15).
 *
 * The Entry is named by its Container and its **entry number**, the 0-based
 * position in that Container's entry table, which a rename leaves unchanged
 * (spec: FM-9).
 * The old name is not carried: it is the catalog's.
 */
export interface EntryRename {
  /** The Container holding the renamed Entry. */
  containerId: ContainerId;
  /** The renamed Entry's position in that Container's entry table. */
  entryNumber: number;
  /** The Entry Path the Entry stands at once the record is applied (NFC, spec: EP-1). */
  path: string;
}

/** Whether `path` occupies the reserved derived namespace (spec: FM-15). */
export function isDerivedPath(path: string): boolean {
  return path.split('/')[0] === DERIVED_COMPONENT;
}
