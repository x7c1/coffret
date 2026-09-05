/**
 * One Entry as the catalog records it (FM-15, FM-16).
 *
 * The same map the meta section writes — `offset`, `size`, `hash`, `mime`, and
 * `derived_from` are shared with it verbatim — except for the values a later
 * rename could move. Those are spelled `path`, `mtime`, and `btime` here,
 * without the `original_` prefix FM-9 gives them, because a Journal record and
 * an Index Snapshot are the catalog's durable form: an addition's values are
 * what the Library holds now, not what one immutable object happened to
 * capture.
 *
 * A Snapshot's entry map carries `container` beside these fields, and the maps
 * are forward-open anyway (FM-9), so anything not asked for here is stepped
 * over rather than refused.
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

/** Serializes one Entry to the map a record and a Snapshot give it. */
export function encodeCatalogEntryMap(entry: EntryMetadata): Map<string, unknown> {
  const map = new Map<string, unknown>([['path', entry.path]]);
  encodeExtent(map, entry, 'control_payload_encode_failed');
  map.set('mtime', entry.mtimeSeconds);
  if (entry.btimeSeconds !== undefined) {
    map.set('btime', entry.btimeSeconds);
  }
  encodeTrailingFields(map, entry);
  return map;
}

/** Reads one Entry out of a catalog map that may carry more. */
export function decodeCatalogEntryMap(map: CborMap, code: CoffretErrorCode): EntryMetadata {
  const entry: EntryMetadata = {
    path: storedPath(map, 'path', code, 'path'),
    ...decodeExtent(map, code),
    mtimeSeconds: requiredInt(map, 'mtime', code),
    hash: decodeHash(map, code),
  };
  const btime = optionalInt(map, 'btime', code);
  if (btime !== undefined) {
    entry.btimeSeconds = btime;
  }
  decodeTrailingFields(entry, map, code);
  return entry;
}
