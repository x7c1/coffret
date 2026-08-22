import {
  asciiBytes,
  bytesEqual,
  readU16BE,
  readU64BE,
  takeExactly,
  writeU16BE,
  writeU64BE,
} from '../internal/bytes.js';
import { NONCE_LENGTH } from '../internal/nonce.js';
import { fail } from '../errors.js';
import { Generation } from '../model/generation.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import type { ControlObjectKind } from '../model/kinds.js';

/** Total length of a control-object header in bytes. */
export const CONTROL_HEADER_LENGTH = 44;

/** The bytes every control object v1 starts with. */
export const CONTROL_MAGIC = asciiBytes('CFCTL');

/** The control-object format version this package writes and reads. */
export const CONTROL_VERSION = 0x01;

const VERSION_OFFSET = 5;
const KIND_OFFSET = 6;
const RESERVED_OFFSET = 7;
const GENERATION_OFFSET = 8;
const REPLICA_INDEX_OFFSET = 16;
const REPLICA_COUNT_OFFSET = 18;
const NONCE_OFFSET = 20;

/** The kind byte each control-object kind is written as (FM-11). */
const KIND_BYTES: Readonly<Record<ControlObjectKind, number>> = {
  journal: 0x01,
  keyring: 0x02,
  'index-snapshot': 0x03,
  'activation-snapshot': 0x04,
};

/**
 * The 44 plaintext bytes every control object starts with (FM-11).
 *
 * ```text
 * offset  size  field
 * ------  ----  -----
 * 0       5     magic = "CFCTL"
 * 5       1     format version = 0x01
 * 6       1     kind (0x01 Journal / 0x02 Keyring / 0x03 Index Snapshot
 *                   / 0x04 activation Index Snapshot)
 * 7       1     reserved = 0x00
 * 8       8     generation
 * 16      2     replica index (0-based)
 * 18      2     replica count
 * 20      24    nonce (random)
 * ```
 *
 * The whole 44 bytes are the associated data of the payload, so editing the
 * kind, the generation, the replica position, or the nonce fails
 * authentication. Unlike a Container, a control object carries its nonce: its
 * purpose key covers every object of that kind, so there is no per-object
 * counter to build a deterministic nonce from.
 */
export interface ControlHeader {
  /** Which kind of control state the payload carries. */
  kind: ControlObjectKind;
  /**
   * Where the object sits in the Library's control history; the numbering
   * never restarts at a rotation (FM-13).
   */
  generation: Generation;
  /** Which replica this is, out of how many. */
  replica: ReplicaPosition;
  /** The nonce the payload was encrypted under. */
  nonce: Uint8Array;
}

/** Serializes the header. Multi-byte integers are big-endian. */
export function encodeControlHeader(header: ControlHeader): Uint8Array {
  const bytes = new Uint8Array(CONTROL_HEADER_LENGTH);
  bytes.set(CONTROL_MAGIC, 0);
  bytes[VERSION_OFFSET] = CONTROL_VERSION;
  bytes[KIND_OFFSET] = KIND_BYTES[header.kind];
  writeU64BE(bytes, GENERATION_OFFSET, header.generation.value);
  writeU16BE(bytes, REPLICA_INDEX_OFFSET, header.replica.index);
  writeU16BE(bytes, REPLICA_COUNT_OFFSET, header.replica.count);
  bytes.set(takeExactly(header.nonce, NONCE_LENGTH, 'a nonce'), NONCE_OFFSET);
  return bytes;
}

/**
 * Reads the header off the front of an object.
 *
 * Every check here is on plaintext bytes, so an object that is not a control
 * object v1 is rejected without a key ever being used.
 */
export function parseControlHeader(object: Uint8Array): ControlHeader {
  if (object.length < CONTROL_HEADER_LENGTH) {
    fail(
      'control_header_too_short',
      `expected at least ${CONTROL_HEADER_LENGTH} control-object header bytes, found ${object.length}`,
    );
  }
  const bytes = object.subarray(0, CONTROL_HEADER_LENGTH);
  if (!bytesEqual(bytes.subarray(0, CONTROL_MAGIC.length), CONTROL_MAGIC)) {
    fail('unknown_control_magic', 'unknown magic, not a control object');
  }
  if (bytes[VERSION_OFFSET] !== CONTROL_VERSION) {
    fail(
      'unsupported_control_version',
      `unsupported control-object format version ${bytes[VERSION_OFFSET]}`,
    );
  }
  const kind = kindFromByte(bytes[KIND_OFFSET]);
  if (bytes[RESERVED_OFFSET] !== 0) {
    fail('reserved_not_zero', 'reserved header bytes are not zero');
  }
  return {
    kind,
    generation: Generation.of(readU64BE(bytes, GENERATION_OFFSET)),
    replica: ReplicaPosition.of(
      readU16BE(bytes, REPLICA_INDEX_OFFSET),
      readU16BE(bytes, REPLICA_COUNT_OFFSET),
    ),
    nonce: Uint8Array.from(bytes.subarray(NONCE_OFFSET, NONCE_OFFSET + NONCE_LENGTH)),
  };
}

/**
 * The kind a kind byte names.
 *
 * A future control-object kind takes a new kind byte, so a byte this build does
 * not know names no kind it can open.
 */
function kindFromByte(byte: number): ControlObjectKind {
  for (const [kind, value] of Object.entries(KIND_BYTES)) {
    if (value === byte) {
      return kind as ControlObjectKind;
    }
  }
  return fail(
    'unknown_control_object_kind',
    `unknown control-object kind 0x${byte.toString(16).padStart(2, '0')}`,
  );
}
