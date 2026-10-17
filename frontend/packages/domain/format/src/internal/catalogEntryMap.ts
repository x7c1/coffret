/**
 * One Entry as the catalog records it (spec: FM-15, FM-16).
 *
 * The same map the meta section writes — `offset`, `size`, `hash`, `mime`, and
 * `derived_from` are shared with it verbatim — except for the values a later
 * rename could move. Those are spelled `path`, `mtime`, and `btime` here,
 * without the `captured_` prefix FM-9 gives them, because a Journal record and
 * an Index Snapshot are the catalog's durable form: an addition's values are
 * what the Library holds now, not what one immutable object happened to
 * capture.
 *
 * The name is the one value that can be carried in both spellings: `path` is
 * where the Entry stands in the Library now, and `captured_path` is the Entry
 * Path the owning Container's meta section records — the name it was captured
 * under, which no rename moves. `captured_path` is written only where it
 * differs from `path`, so a map without it names an Entry whose two paths are
 * one. A map carrying it equal to `path` is a second spelling of that same
 * state and is refused.
 *
 * A Snapshot's entry map carries `container` beside these fields, and the maps
 * are forward-open anyway (spec: FM-9), so anything not asked for here is
 * stepped over rather than refused.
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
import { fail, type CoffretErrorCode } from '../errors.js';
import type { EntryMetadata } from '../model/entry.js';

/** Serializes one Entry to the map a record and a Snapshot give it. */
export function encodeCatalogEntryMap(entry: EntryMetadata): Map<string, unknown> {
  const map = new Map<string, unknown>([['path', entry.path]]);
  // Present only where a rename has set the two apart (spec: FM-15, FM-16), so
  // one state has one encoding.
  if (entry.capturedPath !== entry.path) {
    map.set('captured_path', entry.capturedPath);
  }
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
  const path = storedPath(map, 'path', code, 'path');
  // Absent where the two paths are one, which every map an earlier schema
  // wrote also is (spec: FM-15, FM-16).
  const capturedPath = map.has('captured_path')
    ? storedPath(map, 'captured_path', code, 'captured_path')
    : path;
  if (map.has('captured_path') && capturedPath === path) {
    fail(code, 'an entry map carries a captured_path equal to its path');
  }
  const entry: EntryMetadata = {
    path,
    capturedPath,
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
