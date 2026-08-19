/**
 * The meta section: one CBOR map, encrypted as a single AEAD message (FM-9).
 *
 * Container-level fields are `schema`, `kind`, `pad_len`, and `entries`; each
 * entry records `path`, `offset`, `size`, `mtime`, and `hash`, plus optional
 * `derived_from` and `mime`. The maps are forward-open — a reader ignores fields
 * it does not know, and adding a field only increments `schema`.
 *
 * The plaintext is that map followed by zero padding up to its Padmé bucket, so
 * the length the header records is not a proxy for how many Entries the
 * Container holds or how long their paths are. CBOR is self-delimiting, so
 * nothing records where the map ends: a reader takes one item and then checks
 * that the rest of the plaintext is zero.
 */

import {
  asCborMap,
  decodeCborFirst,
  encodeCbor,
  optionalText,
  requiredArray,
  requiredBytes,
  requiredInt,
  requiredText,
  requiredUint,
  type CborMap,
} from './internal/cbor.js';
import { U64_MAX, isAllZero, takeExactly } from './internal/bytes.js';
import { fail } from './errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from './model/containerId.js';
import { CONTENT_HASH_LENGTH, type DerivedFrom, type EntryMetadata } from './model/entry.js';
import { isContainerKind, type ContainerKind } from './model/kinds.js';

/** The meta section schema this package writes. */
export const META_SCHEMA = 1n;

/** What a decoded meta section says about the Container. */
export interface Meta {
  /** Whether this Container is one-file or a Pack. */
  kind: ContainerKind;
  /** How many zero bytes follow the entries in the plaintext stream (FM-4). */
  padLength: bigint;
  /** The entry table, in plaintext stream order. */
  entries: EntryMetadata[];
}

/** Serializes a meta section to its CBOR plaintext. */
export function encodeMeta(meta: Meta): Uint8Array {
  const map = new Map<string, unknown>([
    ['schema', META_SCHEMA],
    ['kind', meta.kind],
    ['pad_len', meta.padLength],
    ['entries', meta.entries.map(encodeEntry)],
  ]);
  return encodeCbor(map, 'meta_encode_failed');
}

function encodeEntry(entry: EntryMetadata): Map<string, unknown> {
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
 * Parses a meta section from its CBOR plaintext and validates the entry table.
 *
 * The plaintext is one CBOR map followed by a zero-filled padding tail, so this
 * reads exactly one item and then insists that everything after it is zero —
 * the same check the stream's padding tail gets, and what keeps the padding from
 * becoming a place to smuggle bytes past a reader.
 */
export function decodeMeta(plaintext: Uint8Array): Meta {
  const [value, padding] = decodeCborFirst(plaintext, 'malformed_meta');
  if (!isAllZero(padding)) {
    fail('non_zero_meta_padding', 'meta section padding is not zero-filled');
  }
  const map = asCborMap(value, 'malformed_meta', 'the meta section');

  const schema = requiredUint(map, 'schema', 'malformed_meta');
  if (schema < META_SCHEMA) {
    fail('unsupported_meta_schema', `unsupported meta section schema ${schema}`);
  }
  const kind = requiredText(map, 'kind', 'malformed_meta');
  if (!isContainerKind(kind)) {
    fail('malformed_meta', `unknown Container kind ${JSON.stringify(kind)}`);
  }
  const padLength = requiredUint(map, 'pad_len', 'malformed_meta');

  const wireEntries = requiredArray(map, 'entries', 'malformed_meta');
  if (wireEntries.length === 0) {
    fail('empty_entry_table', 'a Container must hold at least one Entry');
  }
  const entries = wireEntries.map((entry, index) =>
    decodeEntry(asCborMap(entry, 'malformed_meta', `entry ${index}`)),
  );

  // The entries must tile the stream from zero without gaps or overlaps: that
  // is what makes `offset` and `size` usable to range-read one Entry.
  let expectedOffset = 0n;
  for (const [index, entry] of entries.entries()) {
    if (entry.offset !== expectedOffset) {
      fail(
        'entry_table_not_contiguous',
        `entry ${index} does not follow its predecessor in the stream`,
      );
    }
    expectedOffset = entry.offset + entry.size;
    if (expectedOffset > U64_MAX) {
      fail('stream_too_long', 'entry sizes overflow the plaintext stream');
    }
  }

  return { kind, padLength, entries };
}

function decodeEntry(map: CborMap): EntryMetadata {
  const entry: EntryMetadata = {
    path: requiredText(map, 'path', 'malformed_meta'),
    offset: requiredUint(map, 'offset', 'malformed_meta'),
    size: requiredUint(map, 'size', 'malformed_meta'),
    mtimeSeconds: requiredInt(map, 'mtime', 'malformed_meta'),
    hash: takeExactly(
      requiredBytes(map, 'hash', 'malformed_meta'),
      CONTENT_HASH_LENGTH,
      'a content hash',
    ),
  };
  const derivedFrom = map.get('derived_from');
  if (derivedFrom !== undefined) {
    entry.derivedFrom = decodeDerivedFrom(
      asCborMap(derivedFrom, 'malformed_meta', 'derived_from'),
    );
  }
  const mime = optionalText(map, 'mime', 'malformed_meta');
  if (mime !== undefined) {
    entry.mime = mime;
  }
  return entry;
}

function decodeDerivedFrom(map: CborMap): DerivedFrom {
  return {
    containerId: ContainerId.fromBytes(
      takeExactly(
        requiredBytes(map, 'container_id', 'malformed_meta'),
        CONTAINER_ID_LENGTH,
        'a Container ID',
      ),
    ),
    path: requiredText(map, 'path', 'malformed_meta'),
  };
}

/**
 * The length of the plaintext stream a meta section describes: every Entry back
 * to back, then the padding tail.
 */
export function plaintextLength(meta: Meta): bigint {
  const last = meta.entries.at(-1);
  const unpadded = last === undefined ? 0n : last.offset + last.size;
  const total = unpadded + meta.padLength;
  if (total > U64_MAX) {
    fail('stream_too_long', 'the plaintext stream is longer than a 64-bit length');
  }
  return total;
}
