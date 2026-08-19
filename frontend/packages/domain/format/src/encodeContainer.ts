import { blake3 } from '@noble/hashes/blake3.js';

import { TAG_LENGTH, seal } from './internal/aead.js';
import { U32_MAX, toLength } from './internal/bytes.js';
import { chunkNonce, metaNonce } from './internal/nonce.js';
import { StreamReader } from './internal/stream.js';
import {
  CONTAINER_HEADER_LENGTH,
  DEFAULT_CHUNK_SIZE,
  encodeContainerHeader,
  requireChunkSize,
} from './containerHeader.js';
import { fail } from './errors.js';
import { encodeMeta, type Meta } from './meta.js';
import { paddedLength } from './padme.js';
import type { ContainerId } from './model/containerId.js';
import type { ContainerKey } from './model/containerKey.js';
import type { EntryMetadata, EntrySource } from './model/entry.js';
import type { ContainerKind } from './model/kinds.js';

/** Everything the encoder needs to lay out one Container. */
export interface ContainerEncodeRequest {
  /** Identifies the Container and names it on Storage. */
  containerId: ContainerId;
  /** Whether this Container is one-file or a Pack. */
  kind: ContainerKind;
  /** The key this Container — and only this Container — is encrypted with. */
  key: ContainerKey;
  /** The entries, in the order they occupy the plaintext stream. */
  entries: readonly EntrySource[];
  /**
   * Plaintext bytes per chunk, recorded in the header for readers to honor.
   *
   * Defaults to [`DEFAULT_CHUNK_SIZE`].
   */
  chunkSize?: number;
}

/** A finished Container: the bytes to upload and the name to upload them under. */
export interface EncodedContainer {
  /** The full object, header first. */
  bytes: Uint8Array;
  /** The name this object is stored under (FM-3). */
  objectName: string;
}

/**
 * Lays out a Container: header, encrypted meta section, encrypted chunks.
 *
 * The plaintext stream is every Entry's content in the order given, padded up to
 * its Padmé bucket; that stream is cut into chunks of the requested size and each
 * chunk is encrypted separately, so the padding tail is never materialized and
 * only one chunk of plaintext is buffered at a time.
 */
export function encodeContainer(request: ContainerEncodeRequest): EncodedContainer {
  // A Container exists only to hold user data (FM-10), so an empty one is not a
  // Container worth writing.
  if (request.entries.length === 0) {
    fail('empty_entry_table', 'a Container must hold at least one Entry');
  }
  const chunkSize = requireChunkSize(request.chunkSize ?? DEFAULT_CHUNK_SIZE);

  const entries: EntryMetadata[] = [];
  let offset = 0n;
  for (const source of request.entries) {
    const size = BigInt(source.content.length);
    const entry: EntryMetadata = {
      path: source.path,
      offset,
      size,
      mtimeSeconds: source.mtimeSeconds,
      hash: blake3(source.content),
    };
    if (source.derivedFrom !== undefined) {
      entry.derivedFrom = source.derivedFrom;
    }
    if (source.mime !== undefined) {
      entry.mime = source.mime;
    }
    entries.push(entry);
    offset += size;
  }

  const unpaddedLength = offset;
  const streamLength = paddedLength(unpaddedLength);
  const meta: Meta = {
    kind: request.kind,
    padLength: streamLength - unpaddedLength,
    entries,
  };

  // The header's associated data covers the meta section length, so the meta
  // section has to be serialized and padded to its Padmé bucket before the
  // header can be written (FM-9).
  const metaMap = encodeMeta(meta);
  const paddedMetaLength = toLength(
    paddedLength(BigInt(metaMap.length)),
    'the padded meta section length',
  );
  const metaPlaintext = new Uint8Array(paddedMetaLength);
  metaPlaintext.set(metaMap, 0);
  const metaLength = paddedMetaLength + TAG_LENGTH;
  if (metaLength > U32_MAX) {
    fail('meta_section_too_long', "meta section exceeds the header's length field");
  }

  const header = encodeContainerHeader({
    containerId: request.containerId,
    chunkSize,
    metaLength,
  });
  const keyBytes = request.key.bytes();
  const metaSection = seal(keyBytes, metaNonce(), header, metaPlaintext);

  // Entries that are all empty still produce one empty final chunk, so every
  // object ends with a final-chunk message marking the end of the stream.
  const chunkSizeBig = BigInt(chunkSize);
  const chunkCount =
    streamLength === 0n ? 1n : (streamLength + chunkSizeBig - 1n) / chunkSizeBig;

  const objectLength = toLength(
    BigInt(CONTAINER_HEADER_LENGTH + metaLength) + streamLength + chunkCount * BigInt(TAG_LENGTH),
    'the Container length',
  );
  const object = new Uint8Array(objectLength);
  object.set(header, 0);
  object.set(metaSection, CONTAINER_HEADER_LENGTH);
  let written = CONTAINER_HEADER_LENGTH + metaLength;

  const reader = new StreamReader(request.entries, meta.padLength);
  // A stream shorter than one chunk needs no more buffer than it fills.
  const bufferLength = Math.max(Number(chunkSizeBig < streamLength ? chunkSizeBig : streamLength), 1);
  const buffer = new Uint8Array(bufferLength);
  for (let index = 0n; index < chunkCount; index += 1n) {
    const filled = reader.read(buffer);
    const isFinal = index + 1n === chunkCount;
    const chunk = seal(keyBytes, chunkNonce(index, isFinal), header, buffer.subarray(0, filled));
    object.set(chunk, written);
    written += chunk.length;
  }

  return { bytes: object, objectName: request.containerId.objectName() };
}
