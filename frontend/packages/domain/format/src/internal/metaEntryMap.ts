/**
 * One row of a Container's entry table, in the meta section's spelling (FM-9).
 *
 * The values a later rename could move carry `original_` names here: what the
 * Entry was called, when it was last modified, and when its file came into
 * being *as of the moment this Container was written*, not the first name the
 * Entry ever had. A Container is one immutable object, so nothing rewrites
 * them; what the Library holds now is the Journal's business, and a record
 * spells the same values `catalogEntryMap.ts`'s way.
 */

import { optionalInt, requiredInt, type CborMap } from './cbor.js';
import {
  decodeExtent,
  decodeHash,
  decodeTrailingFields,
  encodeExtent,
  encodeTrailingFields,
  storedPath,
} from './entryFields.js';
import type { CoffretErrorCode } from '../errors.js';
import type { EntryMetadata } from '../model/entry.js';

/** Serializes one Entry to the map FM-9 gives it. */
export function encodeMetaEntryMap(entry: EntryMetadata): Map<string, unknown> {
  const map = new Map<string, unknown>([['original_path', entry.path]]);
  encodeExtent(map, entry);
  map.set('original_mtime', entry.mtimeSeconds);
  if (entry.btimeSeconds !== undefined) {
    map.set('original_btime', entry.btimeSeconds);
  }
  encodeTrailingFields(map, entry);
  return map;
}

/**
 * Reads one Entry out of a meta section's entry map.
 *
 * The maps are forward-open (FM-9), so anything not asked for here is stepped
 * over rather than refused.
 */
export function decodeMetaEntryMap(map: CborMap, code: CoffretErrorCode): EntryMetadata {
  const entry: EntryMetadata = {
    path: storedPath(map, 'original_path', code, 'original_path'),
    ...decodeExtent(map, code),
    mtimeSeconds: requiredInt(map, 'original_mtime', code),
    hash: decodeHash(map, code),
  };
  const btime = optionalInt(map, 'original_btime', code);
  if (btime !== undefined) {
    entry.btimeSeconds = btime;
  }
  decodeTrailingFields(entry, map, code);
  return entry;
}
