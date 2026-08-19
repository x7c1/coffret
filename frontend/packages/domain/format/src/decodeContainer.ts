import { blake3 } from '@noble/hashes/blake3.js';

import { TAG_LENGTH, open } from './internal/aead.js';
import { bytesEqual, toLength } from './internal/bytes.js';
import { chunkNonce, metaNonce } from './internal/nonce.js';
import { StreamWriter } from './internal/stream.js';
import { CONTAINER_HEADER_LENGTH, parseContainerHeader } from './containerHeader.js';
import { fail } from './errors.js';
import { decodeMeta, plaintextLength } from './meta.js';
import type { ContainerId } from './model/containerId.js';
import type { ContainerKey } from './model/containerKey.js';
import type { DecodedEntry } from './model/entry.js';
import type { ContainerKind } from './model/kinds.js';

/** An opened Container. */
export interface DecodedContainer {
  /** The Container ID from the header. */
  containerId: ContainerId;
  /** The chunk size the object was written with. */
  chunkSize: number;
  /** Whether this Container is one-file or a Pack. */
  kind: ContainerKind;
  /** The entries, in plaintext stream order. */
  entries: DecodedEntry[];
}

/**
 * Opens a Container.
 *
 * The header is validated on its plaintext bytes first, so an object that is not
 * a Container v1 is rejected before the key is used at all. After that every
 * chunk is authenticated before any of its bytes land in an Entry buffer (FM-1,
 * FM-5), and each recovered Entry is checked against its recorded hash.
 */
export function decodeContainer(object: Uint8Array, key: ContainerKey): DecodedContainer {
  const header = parseContainerHeader(object);

  // The associated data is the header exactly as it appears in the object.
  const associatedData = object.subarray(0, CONTAINER_HEADER_LENGTH);
  const body = object.subarray(CONTAINER_HEADER_LENGTH);
  if (body.length < header.metaLength) {
    fail('truncated', "object ends before its header's declared lengths");
  }
  const metaSection = body.subarray(0, header.metaLength);

  const keyBytes = key.bytes();
  const meta = decodeMeta(open(keyBytes, metaNonce(), associatedData, metaSection));
  const expectedLength = plaintextLength(meta);

  const chunks = body.subarray(header.metaLength);
  if (chunks.length === 0) {
    fail('missing_chunks', 'object carries no chunks');
  }
  // Every non-final chunk is exactly one chunk size plus a tag, so the last
  // message in the object is the only one that can be shorter — and the
  // final-chunk domain in its nonce is what a truncated or extended chunk
  // sequence trips over.
  const messageLength = header.chunkSize + TAG_LENGTH;

  // The entry sizes come from the meta section, which is authenticated under
  // the Container Key: only a holder of that key can steer these allocations.
  const writer = new StreamWriter(
    meta.entries.map((entry) => toLength(entry.size, 'an Entry size')),
    meta.padLength,
    expectedLength,
  );

  let offset = 0;
  let index = 0n;
  while (offset < chunks.length) {
    const isFinal = chunks.length - offset <= messageLength;
    const take = isFinal ? chunks.length - offset : messageLength;
    const plaintext = open(
      keyBytes,
      chunkNonce(index, isFinal),
      associatedData,
      chunks.subarray(offset, offset + take),
    );
    writer.write(plaintext);
    offset += take;
    index += 1n;
  }

  if (writer.written !== expectedLength) {
    fail(
      'plaintext_length_mismatch',
      `expected ${expectedLength} plaintext bytes, decrypted ${writer.written}`,
    );
  }

  const contents = writer.contents();
  const entries = meta.entries.map((metadata, entryIndex) => {
    const content = contents[entryIndex];
    if (!bytesEqual(blake3(content), metadata.hash)) {
      fail(
        'content_hash_mismatch',
        `entry ${entryIndex} does not match its recorded content hash`,
      );
    }
    return { metadata, content };
  });

  return {
    containerId: header.containerId,
    chunkSize: header.chunkSize,
    kind: meta.kind,
    entries,
  };
}
