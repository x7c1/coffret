/**
 * The meta section: one CBOR map, encrypted as a single AEAD message (FM-9).
 *
 * Container-level fields are `schema`, `kind`, `pad_len`, and `entries`; each
 * entry records `original_path`, `offset`, `size`, `original_mtime`, and
 * `hash`, plus optional `original_btime`, `derived_from`, and `mime`. The
 * `original_` prefix says what those values are: the Entry Path and the times
 * as of the moment this Container was written, which is all an immutable object
 * can state about them. The maps are forward-open — a reader ignores fields it
 * does not know, and adding a field only increments `schema`.
 *
 * The plaintext is that map followed by zero padding up to its Padmé bucket, so
 * the length the header records is not a proxy for how many Entries the
 * Container holds or how long their paths are. CBOR is self-delimiting, so
 * nothing records where the map ends: a reader takes one item and then holds
 * what is left to that rule — exactly the zero bytes that carry the map to its
 * bucket, and no other length.
 */

import {
  asCborMap,
  decodeCborFirst,
  encodeCbor,
  requiredArray,
  requiredText,
  requiredUint,
} from './internal/cbor.js';
import { decodeMetaEntryMap, encodeMetaEntryMap } from './internal/metaEntryMap.js';
import { U64_MAX, isAllZero } from './internal/bytes.js';
import { fail } from './errors.js';
import { paddedLength } from './padme.js';
import { type EntryMetadata } from './model/entry.js';
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
    ['entries', meta.entries.map(encodeMetaEntryMap)],
  ]);
  return encodeCbor(map, 'meta_encode_failed');
}

/**
 * Parses a meta section from its CBOR plaintext and validates the entry table.
 *
 * The plaintext is one CBOR map followed by a zero-filled padding tail, so this
 * reads exactly one item and then holds what is left to FM-9's padding rule:
 * exactly the zero bytes that carry the map to its Padmé bucket. A non-zero byte
 * would make the padding a place to smuggle bytes past a reader, and any other
 * length was written by something that did not pad as the rule says — which
 * would leave the header's meta section length saying what the map does not.
 */
export function decodeMeta(plaintext: Uint8Array): Meta {
  const [value, padding] = decodeCborFirst(plaintext, 'malformed_meta');
  const expected = paddedLength(BigInt(plaintext.length - padding.length));
  if (BigInt(plaintext.length) !== expected) {
    fail(
      'meta_padding_length_mismatch',
      `expected a meta section padded to ${expected} bytes, found ${plaintext.length}`,
    );
  }
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
  const entries: EntryMetadata[] = wireEntries.map((entry, index) =>
    decodeMetaEntryMap(asCborMap(entry, 'malformed_meta', `entry ${index}`), 'malformed_meta'),
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
