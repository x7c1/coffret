import { encode as encodeCborValue } from 'cborg';
import { describe, expect, it } from 'vitest';

import { errorCode } from './errors.testing.js';
import { decodeMeta, encodeMeta, plaintextLength, type Meta } from './meta.js';
import { ContainerId } from './model/containerId.js';
import type { EntryMetadata } from './model/entry.js';

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
    const decoded = decodeMeta(encodeMeta(sample()));
    expect(decoded).toEqual(sample());
  });

  // FM-9: `kind` carries the explicit Container kind, spelled `one-file` or
  // `pack`.
  it('spells the Container kind as the rule does', () => {
    for (const kind of ['one-file', 'pack'] as const) {
      expect(decodeMeta(encodeMeta({ ...sample(), kind })).kind).toBe(kind);
    }
    const unknownKind = sampleMap();
    unknownKind.set('kind', 'bundle');
    expect(errorCode(() => decodeMeta(encodeCborValue(unknownKind)))).toBe('malformed_meta');
  });

  // FM-9: the plaintext is the CBOR map followed by zero padding, and CBOR is
  // self-delimiting, so a reader takes one item and then insists the rest is
  // zero.
  it('accepts zero padding after the map', () => {
    const unpadded = encodeMeta(sample());
    const padded = new Uint8Array(unpadded.length + 9);
    padded.set(unpadded, 0);
    expect(decodeMeta(padded)).toEqual(decodeMeta(unpadded));
  });

  // FM-9: any non-zero byte after the CBOR map fails decode.
  it('rejects a non-zero byte after the map', () => {
    const unpadded = encodeMeta(sample());
    for (let index = 0; index < 9; index++) {
      const padded = new Uint8Array(unpadded.length + 9);
      padded.set(unpadded, 0);
      padded[unpadded.length + index] = 0x01;
      expect(errorCode(() => decodeMeta(padded)), `padding byte ${index}`).toBe(
        'non_zero_meta_padding',
      );
    }
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
    const decoded = decodeMeta(encodeCborValue(map));
    expect(decoded.entries).toEqual(sample().entries);
    expect(decoded.padLength).toBe(7n);
  });

  // FM-9: a reader accepts any `schema` of 1 or above and rejects anything
  // lower.
  it('rejects a schema below one', () => {
    const map = sampleMap();
    map.set('schema', 0);
    expect(errorCode(() => decodeMeta(encodeCborValue(map)))).toBe('unsupported_meta_schema');
  });

  // FM-10: the entry table of every Container lists at least one Entry, so a
  // meta section with an empty table is rejected on decode.
  it('rejects an empty entry table', () => {
    const empty = encodeMeta({ kind: 'pack', padLength: 0n, entries: [] });
    expect(errorCode(() => decodeMeta(empty))).toBe('empty_entry_table');
  });

  // FM-9: the entry table tiles the plaintext stream exactly — contiguous from
  // offset 0, without gaps or overlaps.
  it('rejects an entry table with a gap or an overlap', () => {
    const gapped = encodeMeta({
      kind: 'pack',
      padLength: 0n,
      entries: [entry('a.txt', 0n, 4n), entry('b.txt', 5n, 9n)],
    });
    expect(errorCode(() => decodeMeta(gapped))).toBe('entry_table_not_contiguous');

    const overlapping = encodeMeta({
      kind: 'pack',
      padLength: 0n,
      entries: [entry('a.txt', 0n, 4n), entry('b.txt', 3n, 9n)],
    });
    expect(errorCode(() => decodeMeta(overlapping))).toBe('entry_table_not_contiguous');
  });

  // FM-9: `hash` is a BLAKE3-256, so a hash of another length is not one.
  it('rejects a content hash of the wrong length', () => {
    const map = sampleMap();
    (map.get('entries') as Map<string, unknown>[])[0].set('hash', new Uint8Array(31));
    expect(errorCode(() => decodeMeta(encodeCborValue(map)))).toBe('invalid_byte_length');
  });

  // FM-9: a `derived_from` points at a Container ID and an Entry Path.
  it('round-trips derived_from and mime', () => {
    const meta = sample();
    meta.entries[0].mime = 'text/plain';
    meta.entries[0].derivedFrom = {
      containerId: ContainerId.fromBytes(new Uint8Array(16).fill(3)),
      path: 'originals/a.txt',
    };
    expect(decodeMeta(encodeMeta(meta))).toEqual(meta);
  });

  // FM-9: `mtime` is a signed count of seconds, and negative values are legal.
  it('round-trips a negative mtime', () => {
    const meta = sample();
    meta.entries[0].mtimeSeconds = -86_400n;
    expect(decodeMeta(encodeMeta(meta)).entries[0].mtimeSeconds).toBe(-86_400n);
  });

  // FM-4: the stream a meta section describes is every Entry back to back, then
  // the padding tail.
  it('reports the plaintext stream length', () => {
    expect(plaintextLength(sample())).toBe(16n);
  });

  it('rejects a meta section that is not a CBOR map', () => {
    expect(errorCode(() => decodeMeta(encodeCborValue('not a map')))).toBe('malformed_meta');
  });
});
