/**
 * The parts of an entry map both spellings share (FM-9, FM-15, FM-16).
 *
 * A Container's meta section and the control payloads record the same Entry,
 * and all but three of the keys are the same keys: `offset`, `size`, `hash`,
 * `mime`, and `derived_from` are written and read identically wherever the map
 * travels. Only the values a later rename could move are spelled differently —
 * `original_path` / `original_mtime` / `original_btime` in the meta section,
 * where they state what one immutable object captured, against `path` /
 * `mtime` / `btime` in a record and a Snapshot, where they state what the
 * Library holds now. Those live in `metaEntryMap.ts` and `catalogEntryMap.ts`;
 * everything neither spelling changes lives here, so a field added to an Entry
 * cannot arrive in one shape there and another here.
 */

import { MAX_FORMAT_INTEGER, takeExactly } from './bytes.js';
import {
  asCborMap,
  optionalText,
  requiredBytes,
  requiredUint,
  requiredText,
  setUint,
  type CborMap,
} from './cbor.js';
import { fail, type CoffretErrorCode } from '../errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from '../model/containerId.js';
import { CONTENT_HASH_LENGTH, type DerivedFrom, type EntryMetadata } from '../model/entry.js';

/**
 * Writes `offset` and `size` into a map that has just had its path written.
 *
 * FM-19 bounds both, and `code` says which map is being written, so a number no
 * reader would take is refused as the meta section or the control payload that
 * was being built rather than written out for the other side to reject. The
 * pair's end is a stream position and is bounded too, and that one is
 * `decodeExtent`'s verdict rather than an encode failure: one condition reads
 * the same whichever direction meets it — which is what the Rust side's
 * `EntryExtent` says by refusing such a pair before a writer ever holds one.
 */
export function encodeExtent(
  map: Map<string, unknown>,
  entry: EntryMetadata,
  code: CoffretErrorCode,
): void {
  setUint(map, 'offset', entry.offset, code);
  setUint(map, 'size', entry.size, code);
  if (entry.offset + entry.size > MAX_FORMAT_INTEGER) {
    fail(
      'stream_too_long',
      "an entry's extent ends past the last plaintext stream position the format admits",
    );
  }
}

/**
 * Reads `offset` and `size`, which neither spelling renames.
 *
 * FM-9, FM-19: the pair describes a range inside the plaintext stream, whose
 * positions the format bounds at 2^63, so an entry whose `offset + size` runs
 * past that has no end that is a position in the stream and places nothing. It
 * is refused here, where every carrier of the map passes through — a
 * Container's meta section, a Journal record's additions, an Index Snapshot's
 * entries — with the verdict a table whose rows outrun the stream already gets,
 * so one object yields one error whichever check meets it first.
 */
export function decodeExtent(
  map: CborMap,
  code: CoffretErrorCode,
): Pick<EntryMetadata, 'offset' | 'size'> {
  const offset = requiredUint(map, 'offset', code);
  const size = requiredUint(map, 'size', code);
  if (offset + size > MAX_FORMAT_INTEGER) {
    fail(
      'stream_too_long',
      "an entry's extent ends past the last plaintext stream position the format admits",
    );
  }
  return { offset, size };
}

/**
 * Writes `hash` and the two optional fields that follow it.
 *
 * `derived_from` keeps the `original_` prefix inside it whichever map carries
 * it: it names an Entry inside an object already written, and no rename reaches
 * in there.
 */
export function encodeTrailingFields(map: Map<string, unknown>, entry: EntryMetadata): void {
  map.set('hash', takeExactly(entry.hash, CONTENT_HASH_LENGTH, 'a content hash'));
  if (entry.derivedFrom !== undefined) {
    map.set(
      'derived_from',
      new Map<string, unknown>([
        ['container_id', entry.derivedFrom.containerId.bytes()],
        ['original_path', entry.derivedFrom.path],
      ]),
    );
  }
  if (entry.mime !== undefined) {
    map.set('mime', entry.mime);
  }
}

/** Reads the two optional fields that follow `hash`, onto `entry`. */
export function decodeTrailingFields(
  entry: EntryMetadata,
  map: CborMap,
  code: CoffretErrorCode,
): void {
  const derivedFrom = map.get('derived_from');
  if (derivedFrom !== undefined) {
    entry.derivedFrom = decodeDerivedFrom(asCborMap(derivedFrom, code, 'derived_from'), code);
  }
  const mime = optionalText(map, 'mime', code);
  if (mime !== undefined) {
    entry.mime = mime;
  }
}

/** The Entry's content hash, held to the length FM-9 gives it. */
export function decodeHash(map: CborMap, code: CoffretErrorCode): Uint8Array {
  return takeExactly(requiredBytes(map, 'hash', code), CONTENT_HASH_LENGTH, 'a content hash');
}

function decodeDerivedFrom(map: CborMap, code: CoffretErrorCode): DerivedFrom {
  return {
    containerId: ContainerId.fromBytes(
      takeExactly(requiredBytes(map, 'container_id', code), CONTAINER_ID_LENGTH, 'a Container ID'),
    ),
    path: storedPath(map, 'original_path', code, 'derived_from.original_path'),
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
 * `key` is the field it is read from, which differs between the two spellings,
 * and `label` says which field carried it. The path itself stays out of the
 * message: a path is Library content, and this error travels further than the
 * payload does.
 */
export function storedPath(
  map: CborMap,
  key: string,
  code: CoffretErrorCode,
  label: string,
): string {
  const path = requiredText(map, key, code);
  if (path !== path.normalize('NFC')) {
    fail('unnormalized_entry_path', `the ${label} of an entry is not normalized to NFC`);
  }
  return path;
}
