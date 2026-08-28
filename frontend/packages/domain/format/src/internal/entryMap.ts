/**
 * The CBOR map one Entry is recorded as (FM-9).
 *
 * A Container's meta section carries a table of these, and so do the control
 * payloads: a Journal record's additions carry the entry table of each
 * Container they add (CP-11, FM-15), and an Index Snapshot carries every
 * current Entry with the index of the Container holding it (FM-16). All three
 * are the same map, so it is written and read in one place — a field added to
 * an Entry arrives everywhere at once, and cannot arrive in one shape here and
 * another there.
 *
 * Which map is being read only changes the error a malformed field raises, so
 * every caller passes its own code and nothing else differs.
 */

import { takeExactly } from './bytes.js';
import {
  asCborMap,
  optionalText,
  requiredBytes,
  requiredInt,
  requiredText,
  requiredUint,
  type CborMap,
} from './cbor.js';
import { fail, type CoffretErrorCode } from '../errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from '../model/containerId.js';
import { CONTENT_HASH_LENGTH, type DerivedFrom, type EntryMetadata } from '../model/entry.js';

/** Serializes one Entry to the map FM-9 gives it. */
export function encodeEntryMap(entry: EntryMetadata): Map<string, unknown> {
  const map = new Map<string, unknown>([
    ['path', entry.path],
    ['offset', entry.offset],
    ['size', entry.size],
    ['mtime', entry.mtimeSeconds],
    ['hash', takeExactly(entry.hash, CONTENT_HASH_LENGTH, 'a content hash')],
  ]);
  if (entry.derivedFrom !== undefined) {
    map.set(
      'derived_from',
      new Map<string, unknown>([
        ['container_id', entry.derivedFrom.containerId.bytes()],
        ['path', entry.derivedFrom.path],
      ]),
    );
  }
  if (entry.mime !== undefined) {
    map.set('mime', entry.mime);
  }
  return map;
}

/**
 * Reads one Entry out of a map that may carry more.
 *
 * A Snapshot's entry map carries `container` beside FM-9's fields, and the maps
 * are forward-open anyway (FM-9), so anything not asked for here is stepped
 * over rather than refused.
 */
export function decodeEntryMap(map: CborMap, code: CoffretErrorCode): EntryMetadata {
  const entry: EntryMetadata = {
    path: storedPath(map, code, 'path'),
    offset: requiredUint(map, 'offset', code),
    size: requiredUint(map, 'size', code),
    mtimeSeconds: requiredInt(map, 'mtime', code),
    hash: takeExactly(requiredBytes(map, 'hash', code), CONTENT_HASH_LENGTH, 'a content hash'),
  };
  const derivedFrom = map.get('derived_from');
  if (derivedFrom !== undefined) {
    entry.derivedFrom = decodeDerivedFrom(asCborMap(derivedFrom, code, 'derived_from'), code);
  }
  const mime = optionalText(map, 'mime', code);
  if (mime !== undefined) {
    entry.mime = mime;
  }
  return entry;
}

function decodeDerivedFrom(map: CborMap, code: CoffretErrorCode): DerivedFrom {
  return {
    containerId: ContainerId.fromBytes(
      takeExactly(requiredBytes(map, 'container_id', code), CONTAINER_ID_LENGTH, 'a Container ID'),
    ),
    path: storedPath(map, code, 'derived_from.path'),
  };
}

/**
 * One Entry Path out of a decoded map (FM-9).
 *
 * Every Entry Path the Library holds is NFC (EP-1) — text from outside is
 * composed before it ever becomes one — so a decoded path that is not was
 * written by something that did not hold to the rule. It is refused rather than
 * composed: the path is inside the bytes the object's own hash was taken over,
 * and rewriting it here would leave this reader decoding to something other
 * than what was encoded.
 *
 * `label` says which field carried it, and the path itself stays out of the
 * message: a path is Library content, and this error travels further than the
 * payload does.
 */
function storedPath(map: CborMap, code: CoffretErrorCode, label: string): string {
  const path = requiredText(map, 'path', code);
  if (path !== path.normalize('NFC')) {
    fail('unnormalized_entry_path', `the ${label} of an entry is not normalized to NFC`);
  }
  return path;
}
