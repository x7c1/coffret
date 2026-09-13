import { describe, expect, it } from 'vitest';

import {
  CONTAINER_HEADER_LENGTH,
  DEFAULT_CHUNK_SIZE,
  MAX_META_LENGTH,
  encodeContainerHeader,
  parseContainerHeader,
} from './containerHeader.js';
import { decodeContainer } from './decodeContainer.js';
import { encodeContainer } from './encodeContainer.js';
import { errorCode } from './errors.testing.js';
import { TAG_LENGTH, open, seal } from './internal/aead.js';
import { U32_MAX, isAllZero, writeU32BE } from './internal/bytes.js';
import { chunkNonce, metaNonce } from './internal/nonce.js';
import { decodeMeta, encodeMeta } from './meta.js';
import { ContainerId } from './model/containerId.js';
import { ContainerKey } from './model/containerKey.js';
import type { EntrySource } from './model/entry.js';
import { paddedLength } from './padme.js';

const KEY = ContainerKey.fromBytes(new Uint8Array(32).fill(0x42));
const ID = ContainerId.fromHex('00112233445566778899aabbccddeeff');

function entry(path: string, content: string): EntrySource {
  return { path, mtimeSeconds: 1_700_000_000n, content: bytes(content) };
}

function bytes(text: string): Uint8Array {
  return Uint8Array.from(text, (character) => character.charCodeAt(0));
}

function encode(entries: readonly EntrySource[], chunkSize?: number): Uint8Array {
  const request = { containerId: ID, kind: 'pack' as const, key: KEY, entries, chunkSize };
  return encodeContainer(request).bytes;
}

/** The meta section's plaintext, as the reader of the object sees it. */
function metaPlaintext(object: Uint8Array): Uint8Array {
  const header = parseContainerHeader(object);
  return open(
    KEY.bytes(),
    metaNonce(),
    object.subarray(0, CONTAINER_HEADER_LENGTH),
    object.subarray(CONTAINER_HEADER_LENGTH, CONTAINER_HEADER_LENGTH + header.metaLength),
  );
}

/**
 * A header declaring `metaLength`, and nothing behind it.
 *
 * The object is deliberately no longer than its header: whatever a reader does
 * with the declaration, it cannot be reading a meta section that is there.
 *
 * The length is written over the meta length field afterwards rather than
 * passed in, because `encodeContainerHeader` holds it against the ceiling too
 * and would refuse the very declarations these cases are about.
 */
function headerDeclaring(metaLength: number): Uint8Array {
  const bytes = encodeContainerHeader({
    containerId: ID,
    chunkSize: DEFAULT_CHUNK_SIZE,
    metaLength: 0,
  });
  writeU32BE(bytes, 28, metaLength);
  return bytes;
}

/** Replaces the meta section with one sealed over `plaintext`. */
function resealMeta(object: Uint8Array, plaintext: Uint8Array): Uint8Array {
  const header = parseContainerHeader(object);
  const associatedData = object.subarray(0, CONTAINER_HEADER_LENGTH);
  const resealed = seal(KEY.bytes(), metaNonce(), associatedData, plaintext);
  const rest = object.subarray(CONTAINER_HEADER_LENGTH + header.metaLength);
  const rebuilt = new Uint8Array(object.length);
  rebuilt.set(associatedData, 0);
  rebuilt.set(resealed, CONTAINER_HEADER_LENGTH);
  rebuilt.set(rest, CONTAINER_HEADER_LENGTH + resealed.length);
  return rebuilt;
}

describe('Container v1', () => {
  // FM-2: the header is magic "CFRT1", format version 0x01, two reserved bytes,
  // the Container ID, the chunk size, and the meta section length, at those
  // exact offsets, with multi-byte integers big-endian.
  it('lays the header out as the field table says', () => {
    const object = encode([entry('a.txt', 'hello')], 64);
    expect(Array.from(object.subarray(0, 5))).toEqual(Array.from(bytes('CFRT1')));
    expect(object[5]).toBe(0x01);
    expect(Array.from(object.subarray(6, 8))).toEqual([0, 0]);
    expect(Array.from(object.subarray(8, 24))).toEqual(Array.from(ID.bytes()));
    expect(Array.from(object.subarray(24, 28))).toEqual([0, 0, 0, 64]);
    const header = parseContainerHeader(object);
    expect(header.chunkSize).toBe(64);
    expect(header.containerId.toHex()).toBe(ID.toHex());
  });

  // FM-3: a Container's object name is its ID as 32 lowercase hex characters
  // followed by `.cfrt`.
  it('names the object after the Container ID', () => {
    const encoded = encodeContainer({
      containerId: ID,
      kind: 'one-file',
      key: KEY,
      entries: [entry('a.txt', 'hello')],
    });
    expect(encoded.objectName).toBe('00112233445566778899aabbccddeeff.cfrt');
  });

  // FM-2, FM-5, FM-9: a Container round-trips whatever it holds — one Entry or
  // many, spanning one chunk or several, empty content included.
  it('round-trips containers of varying entry counts', () => {
    const cases: EntrySource[][] = [
      [entry('one.txt', 'a')],
      [entry('a.txt', 'hello'), entry('b.bin', 'x'.repeat(300))],
      [entry('empty.txt', ''), entry('after-empty.txt', 'tail')],
      Array.from({ length: 5 }, (_, index) => entry(`pack/${index}.txt`, `${index}`.repeat(40))),
    ];
    for (const entries of cases) {
      for (const chunkSize of [16, 64, undefined]) {
        const decoded = decodeContainer(encode(entries, chunkSize), KEY);
        expect(decoded.kind).toBe('pack');
        expect(decoded.containerId.equals(ID)).toBe(true);
        expect(decoded.entries.map((decodedEntry) => decodedEntry.metadata.path)).toEqual(
          entries.map((source) => source.path),
        );
        expect(decoded.entries.map((decodedEntry) => Array.from(decodedEntry.content))).toEqual(
          entries.map((source) => Array.from(source.content)),
        );
        let offset = 0n;
        for (const [index, decodedEntry] of decoded.entries.entries()) {
          expect(decodedEntry.metadata.offset).toBe(offset);
          expect(decodedEntry.metadata.size).toBe(BigInt(entries[index].content.length));
          offset += decodedEntry.metadata.size;
        }
      }
    }
  });

  // FM-4, FM-5: a Container whose entries are all zero-byte files still has a
  // plaintext stream — an empty one, needing no padding — and it is cut into one
  // empty final chunk, so the object still ends with a final-chunk message
  // marking the end of the stream. FM-5 does not say what an empty stream is cut
  // into, so this is a choice both implementations have to make alike for either
  // to read the other's objects.
  it('round-trips a Container whose entries are all empty', () => {
    const entries = [entry('a.txt', ''), entry('b.txt', '')];
    const object = encode(entries, 64);
    const header = parseContainerHeader(object);
    expect(object.length).toBe(CONTAINER_HEADER_LENGTH + header.metaLength + TAG_LENGTH);

    const decoded = decodeContainer(object, KEY);
    expect(decoded.entries.map((decodedEntry) => decodedEntry.content.length)).toEqual([0, 0]);
    expect(decoded.entries.map((decodedEntry) => decodedEntry.metadata.offset)).toEqual([0n, 0n]);
    expect(decodeMeta(metaPlaintext(object)).padLength).toBe(0n);
  });

  // FM-9: an Entry's optional metadata survives the round trip.
  it('round-trips optional entry metadata', () => {
    const source: EntrySource = {
      path: 'derived/thumb.jpg',
      mtimeSeconds: -1n,
      content: bytes('thumbnail'),
      mime: 'image/jpeg',
      derivedFrom: { containerId: ID, path: 'photos/spring.jpg' },
    };
    const decoded = decodeContainer(encode([source]), KEY);
    const metadata = decoded.entries[0].metadata;
    expect(metadata.mime).toBe('image/jpeg');
    expect(metadata.mtimeSeconds).toBe(-1n);
    expect(metadata.derivedFrom?.path).toBe('photos/spring.jpg');
    expect(metadata.derivedFrom?.containerId.equals(ID)).toBe(true);
  });

  // FM-4: the plaintext stream is padded to its Padmé bucket, and `pad_len`
  // records exactly that padding length.
  it('pads the plaintext stream to its Padmé bucket', () => {
    const entries = [entry('a.bin', 'x'.repeat(1_000))];
    const object = encode(entries, 64);
    const meta = decodeMeta(metaPlaintext(object));
    expect(meta.padLength).toBe(paddedLength(1_000n) - 1_000n);
    expect(meta.padLength).toBe(24n);
  });

  // FM-2, FM-9: the meta section length in the header is the padded ciphertext
  // length — the Padmé bucket of the CBOR map plus the AEAD tag.
  it('records the padded meta ciphertext length in the header', () => {
    const object = encode([entry('a.txt', 'hello'), entry('b.txt', 'world')], 64);
    const header = parseContainerHeader(object);
    const plaintext = metaPlaintext(object);
    const map = encodeMeta(decodeMeta(plaintext));

    expect(BigInt(plaintext.length)).toBe(paddedLength(BigInt(map.length)));
    expect(header.metaLength).toBe(plaintext.length + TAG_LENGTH);
    expect(plaintext.length).toBeGreaterThan(map.length);
    expect(isAllZero(plaintext.subarray(map.length))).toBe(true);
  });

  // FM-9: any non-zero byte after the CBOR map fails decode, so the padding is
  // not a place to smuggle bytes past a reader.
  it('rejects a non-zero byte after the meta section CBOR item', () => {
    const object = encode([entry('a.txt', 'hello')], 64);
    const plaintext = metaPlaintext(object);
    const map = encodeMeta(decodeMeta(plaintext));
    for (let index = map.length; index < plaintext.length; index++) {
      const tampered = Uint8Array.from(plaintext);
      tampered[index] = 0x01;
      expect(
        errorCode(() => decodeContainer(resealMeta(object, tampered), KEY)),
        `byte ${index} of the meta padding was not checked`,
      ).toBe('non_zero_meta_padding');
    }
  });

  // FM-4: the stream's padding tail is zero, and a decoder verifies it.
  it('rejects a non-zero byte in the stream padding tail', () => {
    const entries = [entry('a.bin', 'x'.repeat(9))];
    const object = encode(entries, 64);
    const header = parseContainerHeader(object);
    const chunkStart = CONTAINER_HEADER_LENGTH + header.metaLength;
    const associatedData = object.subarray(0, CONTAINER_HEADER_LENGTH);
    // One chunk holds the whole padded stream here, so re-sealing it with a
    // non-zero padding byte produces an object that authenticates but must
    // still be refused.
    const stream = Uint8Array.from(bytes('x'.repeat(9)));
    const padded = new Uint8Array(Number(paddedLength(9n)));
    padded.set(stream, 0);
    padded[padded.length - 1] = 0xff;
    const resealed = seal(KEY.bytes(), chunkNonce(0n, true), associatedData, padded);
    const rebuilt = new Uint8Array(chunkStart + resealed.length);
    rebuilt.set(object.subarray(0, chunkStart), 0);
    rebuilt.set(resealed, chunkStart);
    expect(errorCode(() => decodeContainer(rebuilt, KEY))).toBe('non_zero_padding');
  });

  // FM-10: a Container exists only to hold user data, so an empty one is not a
  // Container worth writing.
  it('refuses to write a Container with no entries', () => {
    expect(errorCode(() => encode([]))).toBe('empty_entry_table');
  });

  // FM-2: an object with an unknown magic or format version is rejected without
  // attempting decryption, and reserved bytes must be zero.
  it('rejects an object that is not a Container v1', () => {
    const object = encode([entry('a.txt', 'hello')], 64);
    expect(errorCode(() => decodeContainer(object.subarray(0, 8), KEY))).toBe('header_too_short');

    const wrongMagic = Uint8Array.from(object);
    wrongMagic[0] = 0x00;
    expect(errorCode(() => decodeContainer(wrongMagic, KEY))).toBe('unknown_magic');

    const wrongVersion = Uint8Array.from(object);
    wrongVersion[5] = 0x02;
    expect(errorCode(() => decodeContainer(wrongVersion, KEY))).toBe('unsupported_version');

    const reserved = Uint8Array.from(object);
    reserved[6] = 0x01;
    expect(errorCode(() => decodeContainer(reserved, KEY))).toBe('reserved_not_zero');

    const zeroChunkSize = Uint8Array.from(object);
    zeroChunkSize.set([0, 0, 0, 0], 24);
    expect(errorCode(() => decodeContainer(zeroChunkSize, KEY))).toBe('invalid_chunk_size');
  });

  // FM-2: a declaration past the ceiling is answered as the header is parsed,
  // for the price of the four bytes it took to read.
  it('refuses a header declaring a meta section past the ceiling', () => {
    expect(errorCode(() => decodeContainer(headerDeclaring(MAX_META_LENGTH + 1), KEY))).toBe(
      'meta_section_too_long',
    );

    // The largest the field can spell is what four edited bytes reach for, and
    // it is sixty-four times the ceiling.
    expect(errorCode(() => decodeContainer(headerDeclaring(U32_MAX), KEY))).toBe(
      'meta_section_too_long',
    );
  });

  // The ceiling is a boundary and not an approximation: the length at it is one
  // a Container may declare, and one byte more is not.
  it('takes the ceiling itself as a length a Container may declare', () => {
    expect(parseContainerHeader(headerDeclaring(MAX_META_LENGTH)).metaLength).toBe(MAX_META_LENGTH);
    expect(errorCode(() => parseContainerHeader(headerDeclaring(MAX_META_LENGTH + 1)))).toBe(
      'meta_section_too_long',
    );
  });

  // FM-2: the ceiling binds the writer too, so every Container this package
  // writes is one a conforming reader takes. The entry table below reaches the
  // ceiling through a handful of absurd Entry Paths rather than through the half
  // a million Entries it would otherwise take; what the table is made of is not
  // what the ceiling is about.
  it('refuses to write a meta section past the ceiling', () => {
    const oversized = Array.from({ length: 64 }, (_, index) => ({
      path: `d${index}/${'a'.repeat(1024 * 1024)}`,
      mtimeSeconds: 1_700_000_000n,
      content: new Uint8Array(0),
    }));
    expect(errorCode(() => encode(oversized))).toBe('meta_section_too_long');

    // And the meta section of a Container anyone would actually write is nowhere
    // near it: the ceiling bounds the absurd, not the ordinary.
    const object = encode([entry('albums/2024/spring.jpg', 'the bytes of one Entry')], 64);
    expect(parseContainerHeader(object).metaLength * 1000).toBeLessThan(MAX_META_LENGTH);
  });

  // The ceiling is what `meta_section_too_long` answers, and it is all it
  // answers. A length that is no byte count at all is a caller's mistake rather
  // than a Container that outgrew the format — a different fault reaching a
  // different caller — so it comes back under its own code, and whoever branches
  // on `meta_section_too_long` never has to ask which of the two it met.
  it('does not answer a meta length that is no byte count with the ceiling refusal', () => {
    for (const nonsense of [-1, 1.5, Number.NaN]) {
      expect(
        errorCode(() =>
          encodeContainerHeader({
            containerId: ID,
            chunkSize: DEFAULT_CHUNK_SIZE,
            metaLength: nonsense,
          }),
        ),
      ).toBe('value_out_of_range');
    }
  });

  // FM-8: the associated data of the meta section and of every chunk is the full
  // 32-byte header, so altering the Container ID, chunk size, or meta section
  // length fails decryption.
  it('binds every message to the header it was written under', () => {
    const object = encode([entry('a.txt', 'hello')], 64);
    // Editing a length or the chunk size can make the object malformed before
    // it is inauthentic; either way none of its plaintext is released.
    const refusals = ['authentication_failed', 'truncated'];
    for (const index of [8, 23, 24, 27, 28, 31]) {
      const tampered = Uint8Array.from(object);
      tampered[index] ^= 0x01;
      expect(refusals, `byte ${index}`).toContain(errorCode(() => decodeContainer(tampered, KEY)));
    }
  });

  // FM-1: a message that fails authentication is rejected whole — a flipped bit
  // anywhere in the ciphertext fails.
  it('rejects a tampered ciphertext byte', () => {
    const object = encode([entry('a.txt', 'hello world')], 16);
    for (let index = CONTAINER_HEADER_LENGTH; index < object.length; index++) {
      const tampered = Uint8Array.from(object);
      tampered[index] ^= 0x01;
      expect(errorCode(() => decodeContainer(tampered, KEY)), `byte ${index}`).toBe(
        'authentication_failed',
      );
    }
  });

  // FM-7: the counter and the final-chunk domain make reordering, truncation,
  // and extension of the chunk sequence fail authentication.
  it('rejects a reordered, truncated, or extended chunk sequence', () => {
    const object = encode([entry('a.bin', 'x'.repeat(100))], 16);
    const header = parseContainerHeader(object);
    const chunkStart = CONTAINER_HEADER_LENGTH + header.metaLength;
    const messageLength = header.chunkSize + TAG_LENGTH;

    const swapped = Uint8Array.from(object);
    const first = object.subarray(chunkStart, chunkStart + messageLength);
    const second = object.subarray(chunkStart + messageLength, chunkStart + 2 * messageLength);
    swapped.set(second, chunkStart);
    swapped.set(first, chunkStart + messageLength);
    expect(errorCode(() => decodeContainer(swapped, KEY))).toBe('authentication_failed');

    const truncated = object.subarray(0, object.length - messageLength);
    expect(errorCode(() => decodeContainer(truncated, KEY))).toBe('authentication_failed');

    const extended = new Uint8Array(object.length + messageLength);
    extended.set(object, 0);
    extended.set(object.subarray(chunkStart, chunkStart + messageLength), object.length);
    expect(errorCode(() => decodeContainer(extended, KEY))).toBe('authentication_failed');
  });

  // FM-1: a Container opens under its own key and under no other.
  it('does not open under another Container Key', () => {
    const object = encode([entry('a.txt', 'hello')], 64);
    const other = ContainerKey.fromBytes(new Uint8Array(32).fill(0x43));
    expect(errorCode(() => decodeContainer(object, other))).toBe('authentication_failed');
  });

  // FM-6: the chunk size is a per-Container parameter a reader honors as
  // recorded, not a constant it assumes.
  it('honors the recorded chunk size', () => {
    for (const chunkSize of [1, 7, 4096]) {
      const decoded = decodeContainer(encode([entry('a.bin', 'x'.repeat(50))], chunkSize), KEY);
      expect(decoded.chunkSize).toBe(chunkSize);
      expect(Array.from(decoded.entries[0].content)).toEqual(Array.from(bytes('x'.repeat(50))));
    }
  });
});
