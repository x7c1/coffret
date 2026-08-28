import { encode as encodeCborValue } from 'cborg';
import { describe, expect, it } from 'vitest';

import { errorCode } from './errors.testing.js';
import { decodeMeta, encodeMeta, plaintextLength, type Meta } from './meta.js';
import { paddedLength } from './padme.js';
import { ContainerId } from './model/containerId.js';
import type { EntryMetadata } from './model/entry.js';

/**
 * A CBOR map carried to its Padmé bucket, which is the plaintext a meta section
 * is stored as (FM-9). A reader holds every plaintext to that length, so a case
 * that hands it a bare map is testing the padding rule rather than its own
 * subject.
 */
function padded(map: Uint8Array): Uint8Array {
  const plaintext = new Uint8Array(Number(paddedLength(BigInt(map.length))));
  plaintext.set(map, 0);
  return plaintext;
}

/** The sample's plaintext, and where its CBOR map ends inside it. */
function samplePlaintext(): [Uint8Array, number] {
  const map = encodeMeta(sample());
  return [padded(map), map.length];
}

/**
 * `café.txt` with the accent as `e` and a combining acute — a spelling no writer
 * holding to EP-1 ever puts in an entry table.
 */
const DECOMPOSED = 'cafe\u0301.txt';

function entry(path: string, offset: bigint, size: bigint): EntryMetadata {
  return {
    path,
    offset,
    size,
    mtimeSeconds: 42n,
    hash: new Uint8Array(32).fill(7),
  };
}

function sample(): Meta {
  return {
    kind: 'pack',
    padLength: 7n,
    entries: [entry('a.txt', 0n, 4n), entry('b.txt', 4n, 5n)],
  };
}

/** The sample as the CBOR map a writer would produce, ready to be edited. */
function sampleMap(): Map<string, unknown> {
  return new Map<string, unknown>([
    ['schema', 1],
    ['kind', 'pack'],
    ['pad_len', 7],
    [
      'entries',
      sample().entries.map(
        (source) =>
          new Map<string, unknown>([
            ['path', source.path],
            ['offset', source.offset],
            ['size', source.size],
            ['mtime', source.mtimeSeconds],
            ['hash', source.hash],
          ]),
      ),
    ],
  ]);
}

describe('the meta section', () => {
  // FM-9: the meta section is one CBOR map with `schema`, `kind`, `pad_len`, and
  // `entries`; each entry records `path`, `offset`, `size`, `mtime`, and `hash`.
  it('round-trips the fields the rule names', () => {
    const decoded = decodeMeta(padded(encodeMeta(sample())));
    expect(decoded).toEqual(sample());
  });

  // FM-9: `kind` carries the explicit Container kind, spelled `one-file` or
  // `pack`.
  it('spells the Container kind as the rule does', () => {
    for (const kind of ['one-file', 'pack'] as const) {
      expect(decodeMeta(padded(encodeMeta({ ...sample(), kind }))).kind).toBe(kind);
    }
    const unknownKind = sampleMap();
    unknownKind.set('kind', 'bundle');
    expect(errorCode(() => decodeMeta(padded(encodeCborValue(unknownKind))))).toBe('malformed_meta');
  });

  // FM-9: the plaintext is the CBOR map followed by zero padding to the map's
  // Padmé bucket, and CBOR is self-delimiting, so a reader takes one item and
  // holds the rest to that rule.
  it('accepts a map padded to its bucket', () => {
    const [plaintext, mapLength] = samplePlaintext();
    expect(mapLength).toBeLessThan(plaintext.length);
    expect(decodeMeta(plaintext)).toEqual(sample());
  });

  // FM-9: any non-zero byte after the CBOR map fails decode, so the padding is
  // not a place to smuggle bytes past a reader.
  it('rejects a non-zero byte after the map', () => {
    const [plaintext, mapLength] = samplePlaintext();
    for (let index = mapLength; index < plaintext.length; index++) {
      const tampered = Uint8Array.from(plaintext);
      tampered[index] = 0x01;
      expect(errorCode(() => decodeMeta(tampered)), `padding byte ${index}`).toBe(
        'non_zero_meta_padding',
      );
    }
  });

  // FM-9: the plaintext is the map and its padding and nothing else, so a zero
  // byte beyond the bucket is a length no writer following the rule produces —
  // and it would put the header's meta section length past what the map
  // accounts for (FM-2).
  it('rejects a plaintext longer than the bucket', () => {
    const [plaintext] = samplePlaintext();
    const overlong = new Uint8Array(plaintext.length + 1);
    overlong.set(plaintext, 0);
    expect(errorCode(() => decodeMeta(overlong))).toBe('meta_padding_length_mismatch');
  });

  // FM-9: a writer that skipped the padding leaks the size the padding exists
  // to blur, so its object is refused rather than quietly read.
  it('rejects an unpadded map', () => {
    const [plaintext, mapLength] = samplePlaintext();
    expect(errorCode(() => decodeMeta(plaintext.subarray(0, mapLength)))).toBe(
      'meta_padding_length_mismatch',
    );
  });

  // FM-9: the maps are forward-open — a reader ignores fields it does not know,
  // so a newer writer can add fields without breaking this reader.
  it('ignores unknown fields', () => {
    const map = sampleMap();
    map.set('schema', 2);
    map.set('future_field', 'whatever');
    for (const wireEntry of map.get('entries') as Map<string, unknown>[]) {
      wireEntry.set('future_entry_field', 1);
    }
    const decoded = decodeMeta(padded(encodeCborValue(map)));
    expect(decoded.entries).toEqual(sample().entries);
    expect(decoded.padLength).toBe(7n);
  });

  // FM-9: a reader accepts any `schema` of 1 or above and rejects anything
  // lower.
  it('rejects a schema below one', () => {
    const map = sampleMap();
    map.set('schema', 0);
    expect(errorCode(() => decodeMeta(padded(encodeCborValue(map))))).toBe('unsupported_meta_schema');
  });

  // FM-10: the entry table of every Container lists at least one Entry, so a
  // meta section with an empty table is rejected on decode.
  it('rejects an empty entry table', () => {
    const empty = encodeMeta({ kind: 'pack', padLength: 0n, entries: [] });
    expect(errorCode(() => decodeMeta(padded(empty)))).toBe('empty_entry_table');
  });

  // FM-9: the entry table tiles the plaintext stream exactly — contiguous from
  // offset 0, without gaps or overlaps.
  it('rejects an entry table with a gap or an overlap', () => {
    const gapped = encodeMeta({
      kind: 'pack',
      padLength: 0n,
      entries: [entry('a.txt', 0n, 4n), entry('b.txt', 5n, 9n)],
    });
    expect(errorCode(() => decodeMeta(padded(gapped)))).toBe('entry_table_not_contiguous');

    const overlapping = encodeMeta({
      kind: 'pack',
      padLength: 0n,
      entries: [entry('a.txt', 0n, 4n), entry('b.txt', 3n, 9n)],
    });
    expect(errorCode(() => decodeMeta(padded(overlapping)))).toBe('entry_table_not_contiguous');
  });

  // FM-9: `hash` is a BLAKE3-256, so a hash of another length is not one.
  it('rejects a content hash of the wrong length', () => {
    const map = sampleMap();
    (map.get('entries') as Map<string, unknown>[])[0].set('hash', new Uint8Array(31));
    expect(errorCode(() => decodeMeta(padded(encodeCborValue(map))))).toBe('invalid_byte_length');
  });

  // FM-9: a `derived_from` points at a Container ID and an Entry Path.
  it('round-trips derived_from and mime', () => {
    const meta = sample();
    meta.entries[0].mime = 'text/plain';
    meta.entries[0].derivedFrom = {
      containerId: ContainerId.fromBytes(new Uint8Array(16).fill(3)),
      path: 'originals/a.txt',
    };
    expect(decodeMeta(padded(encodeMeta(meta)))).toEqual(meta);
  });

  // EP-1: the paths in a meta section are ones the Library already holds, so a
  // decomposed one is a malformed payload and the object is refused rather than
  // composed on the way back in.
  it('rejects an Entry Path that is not in NFC', () => {
    const map = sampleMap();
    (map.get('entries') as Map<string, unknown>[])[0].set('path', DECOMPOSED);
    expect(errorCode(() => decodeMeta(padded(encodeCborValue(map))))).toBe(
      'unnormalized_entry_path',
    );
  });

  // The same rule reaches the path inside a `derived_from` reference, which
  // names an Entry of the Library just as much as the entry's own path does.
  it('rejects a derived_from path that is not in NFC', () => {
    const map = sampleMap();
    (map.get('entries') as Map<string, unknown>[])[0].set(
      'derived_from',
      new Map<string, unknown>([
        ['container_id', new Uint8Array(16).fill(3)],
        ['path', DECOMPOSED],
      ]),
    );
    expect(errorCode(() => decodeMeta(padded(encodeCborValue(map))))).toBe(
      'unnormalized_entry_path',
    );
  });

  // FM-9: `mtime` is a signed count of seconds, and negative values are legal.
  it('round-trips a negative mtime', () => {
    const meta = sample();
    meta.entries[0].mtimeSeconds = -86_400n;
    expect(decodeMeta(padded(encodeMeta(meta))).entries[0].mtimeSeconds).toBe(-86_400n);
  });

  // FM-4: the stream a meta section describes is every Entry back to back, then
  // the padding tail.
  it('reports the plaintext stream length', () => {
    expect(plaintextLength(sample())).toBe(16n);
  });

  it('rejects a meta section that is not a CBOR map', () => {
    expect(errorCode(() => decodeMeta(padded(encodeCborValue('not a map'))))).toBe('malformed_meta');
  });
});
