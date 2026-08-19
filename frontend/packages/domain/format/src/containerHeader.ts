import {
  U32_MAX,
  asciiBytes,
  bytesEqual,
  isAllZero,
  readU32BE,
  writeU32BE,
} from './internal/bytes.js';
import { fail } from './errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from './model/containerId.js';

/** Total length of a Container header in bytes. */
export const CONTAINER_HEADER_LENGTH = 32;

/** The bytes every Container v1 object starts with. */
export const CONTAINER_MAGIC = asciiBytes('CFRT1');

/** The Container format version this package writes and reads. */
export const CONTAINER_VERSION = 0x01;

/** The chunk size new Containers are written with: 1 MiB (FM-6). */
export const DEFAULT_CHUNK_SIZE = 1024 * 1024;

const VERSION_OFFSET = 5;
const RESERVED_OFFSET = 6;
const CONTAINER_ID_OFFSET = 8;
const CHUNK_SIZE_OFFSET = 24;
const META_LENGTH_OFFSET = 28;

/**
 * The 32 plaintext bytes every Container starts with (FM-2).
 *
 * ```text
 * offset  size  field
 * ------  ----  -----
 * 0       5     magic = "CFRT1"
 * 5       1     format version = 0x01
 * 6       2     reserved = 0x0000
 * 8       16    Container ID
 * 24      4     chunk size (plaintext bytes per chunk)
 * 28      4     meta section length M (padded ciphertext bytes)
 * ```
 *
 * The header carries no key material — Key Envelopes live in the Keyring — so
 * rotating the Master Key leaves every Container byte-for-byte unchanged. The
 * whole 32 bytes are the associated data of every AEAD message in the object
 * (FM-8), which is what binds the meta section and the chunks to this exact
 * header.
 */
export interface ContainerHeader {
  /** Identifies the Container and names it on Storage. */
  containerId: ContainerId;
  /** Plaintext bytes per chunk, honored by readers as recorded. */
  chunkSize: number;
  /** Length of the encrypted meta section in bytes, tag included. */
  metaLength: number;
}

/**
 * Insists that a chunk size is one that cuts a stream and fits its header
 * field.
 *
 * The chunk size is a per-Container parameter recorded in the header, not a
 * format constant: a new Container may adopt a different size without a format
 * version change, and a reader always honors the value it finds in the header
 * rather than assuming the default (FM-6).
 */
export function requireChunkSize(chunkSize: number): number {
  if (!Number.isInteger(chunkSize) || chunkSize < 1 || chunkSize > U32_MAX) {
    fail('invalid_chunk_size', `a chunk size is 1 to ${U32_MAX} bytes, found ${chunkSize}`);
  }
  return chunkSize;
}

/** Serializes the header. Multi-byte integers are big-endian. */
export function encodeContainerHeader(header: ContainerHeader): Uint8Array {
  const bytes = new Uint8Array(CONTAINER_HEADER_LENGTH);
  bytes.set(CONTAINER_MAGIC, 0);
  bytes[VERSION_OFFSET] = CONTAINER_VERSION;
  bytes.set(header.containerId.bytes(), CONTAINER_ID_OFFSET);
  writeU32BE(bytes, CHUNK_SIZE_OFFSET, requireChunkSize(header.chunkSize));
  writeU32BE(bytes, META_LENGTH_OFFSET, header.metaLength);
  return bytes;
}

/**
 * Reads the header off the front of an object.
 *
 * Every check here is on plaintext bytes, so an object that is not a Container
 * v1 is rejected without a key ever being used.
 */
export function parseContainerHeader(object: Uint8Array): ContainerHeader {
  if (object.length < CONTAINER_HEADER_LENGTH) {
    fail(
      'header_too_short',
      `expected at least ${CONTAINER_HEADER_LENGTH} header bytes, found ${object.length}`,
    );
  }
  const bytes = object.subarray(0, CONTAINER_HEADER_LENGTH);
  if (!bytesEqual(bytes.subarray(0, CONTAINER_MAGIC.length), CONTAINER_MAGIC)) {
    fail('unknown_magic', 'unknown magic, not a Container');
  }
  if (bytes[VERSION_OFFSET] !== CONTAINER_VERSION) {
    fail('unsupported_version', `unsupported Container format version ${bytes[VERSION_OFFSET]}`);
  }
  if (!isAllZero(bytes.subarray(RESERVED_OFFSET, CONTAINER_ID_OFFSET))) {
    fail('reserved_not_zero', 'reserved header bytes are not zero');
  }
  return {
    containerId: ContainerId.fromBytes(
      bytes.subarray(CONTAINER_ID_OFFSET, CONTAINER_ID_OFFSET + CONTAINER_ID_LENGTH),
    ),
    chunkSize: requireChunkSize(readU32BE(bytes, CHUNK_SIZE_OFFSET)),
    metaLength: readU32BE(bytes, META_LENGTH_OFFSET),
  };
}
