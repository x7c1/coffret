/**
 * One row of a Container's entry table, under the meta section's field names
 * (spec: FM-9). On the wire each field travels under its field number
 * (`fieldNumbers.ts`).
 *
 * The values the Library may later hold differently carry `captured_` names
 * here: what the Entry was called, when it was last modified, and when its file
 * came into being *as captured when this Container was written*, not the first
 * name the Entry ever had. A Container is one immutable object, so nothing
 * rewrites them; what the Library holds now is the Journal's business, and a
 * record spells the same values `catalogEntryMap.ts`'s way.
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
  const map = new Map<string, unknown>([['captured_path', entry.capturedPath]]);
  encodeExtent(map, entry, 'meta_encode_failed');
  map.set('captured_mtime', entry.mtimeSeconds);
  if (entry.btimeSeconds !== undefined) {
    map.set('captured_btime', entry.btimeSeconds);
  }
  encodeTrailingFields(map, entry);
  return map;
}

/**
 * Reads one Entry out of a meta section's entry map.
 *
 * The maps are forward-open (spec: FM-9), so anything not asked for here is
 * stepped over rather than refused.
 */
export function decodeMetaEntryMap(map: CborMap, code: CoffretErrorCode): EntryMetadata {
  const capturedPath = storedPath(map, 'captured_path', code, 'captured_path');
  const entry: EntryMetadata = {
    // What this Container records is the name the Entry had when it was
    // written, and that is all a meta section can say about where it stands.
    path: capturedPath,
    capturedPath,
    ...decodeExtent(map, code),
    mtimeSeconds: requiredInt(map, 'captured_mtime', code),
    hash: decodeHash(map, code),
  };
  const btime = optionalInt(map, 'captured_btime', code);
  if (btime !== undefined) {
    entry.btimeSeconds = btime;
  }
  decodeTrailingFields(entry, map, code);
  return entry;
}
