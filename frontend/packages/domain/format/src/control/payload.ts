/**
 * What a control object carries inside its AEAD message.
 *
 * The payload is one CBOR map, encrypted as that map followed by zero padding up
 * to its Padmé bucket (FM-11): a control object is one AEAD message, so its
 * stored length is its payload's length, and an unpadded payload would count out
 * for the provider whatever the payload lists — the Entries an Index Snapshot
 * names, the Containers a Keyring maps. This is the meta section's rule (FM-9)
 * applied to control objects.
 *
 * This module owns exactly one of the map's fields — `master_key_epoch`, which
 * every control object carries whatever its kind
 * (FM-13) — and treats the rest as the kind's own business: the caller hands
 * over the CBOR map of its fields, and gets those fields back on the way out.
 * The bytes they come back as need not be the bytes they went in as, for the
 * reason [`ControlPayload.body`] gives.
 */

import {
  asCborMap,
  asUint,
  decodeCborExact,
  decodeCborFirst,
  encodeCbor,
} from '../internal/cbor.js';
import { isAllZero } from '../internal/bytes.js';
import { fail } from '../errors.js';
import { paddedLength } from '../padme.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';

/** The one payload field the framing itself defines. */
const MASTER_KEY_EPOCH = 'master_key_epoch';

/** A control-object payload: the epoch that wrote it, and the kind's own map. */
export interface ControlPayload {
  /** The Master Key epoch that encrypted this object. */
  masterKeyEpoch: MasterKeyEpoch;
  /**
   * The kind's own fields, as the CBOR map they were serialized to.
   *
   * On the way out the map is re-encoded rather than sliced out of the payload,
   * so a decoded body carries the fields the writer wrote but not necessarily
   * the bytes it wrote them as — this package's encoder orders map keys
   * canonically and a writer need not have. Bodies from two implementations are
   * compared as decoded maps, never as byte strings.
   */
  body: Uint8Array;
}

/** The CBOR spelling of a map with no entries, for a kind with no fields yet. */
export function emptyPayloadBody(): Uint8Array {
  return encodeCbor(new Map(), 'control_payload_encode_failed');
}

/**
 * Serializes a payload to the plaintext that gets encrypted: the kind's own map
 * with `master_key_epoch` added, then zero padding to its Padmé bucket (FM-11).
 */
export function encodeControlPayload(payload: ControlPayload): Uint8Array {
  const map = readPayloadMap(payload.body);
  if (map.has(MASTER_KEY_EPOCH)) {
    fail('malformed_control_payload', `the body already carries ${MASTER_KEY_EPOCH}`);
  }
  map.set(MASTER_KEY_EPOCH, payload.masterKeyEpoch.value);
  return padToBucket(encodeCbor(map, 'control_payload_encode_failed'));
}

/** Grows a payload map to its Padmé bucket with zero bytes (FM-4, FM-11). */
function padToBucket(map: Uint8Array): Uint8Array {
  const padded = paddedLength(BigInt(map.length));
  // Not the `toLength` helper: its `value_out_of_range` is the generic "this
  // reader cannot address that" of a length read off the wire, and this is a
  // length the padding rule itself asks for, which the Rust side raises as
  // `ControlPayloadTooLong`. Both implementations answer it with the same code.
  if (padded > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail(
      'control_payload_too_long',
      `a control-object payload padded to ${padded} bytes is longer than this runtime addresses`,
    );
  }
  const plaintext = new Uint8Array(Number(padded));
  plaintext.set(map, 0);
  return plaintext;
}

/**
 * Parses a payload plaintext, insisting that it says which epoch encrypted it.
 */
export function decodeControlPayload(plaintext: Uint8Array): ControlPayload {
  const map = readPaddedPayloadMap(plaintext);
  if (!map.has(MASTER_KEY_EPOCH)) {
    fail('missing_master_key_epoch', 'control-object payload carries no master_key_epoch');
  }
  const epoch = asUint(map.get(MASTER_KEY_EPOCH), MASTER_KEY_EPOCH, 'malformed_control_payload');
  map.delete(MASTER_KEY_EPOCH);
  return {
    masterKeyEpoch: MasterKeyEpoch.of(epoch),
    body: encodeCbor(map, 'control_payload_encode_failed'),
  };
}

/**
 * Reads the map a payload plaintext carries, holding what follows it to FM-11's
 * padding rule.
 *
 * CBOR is self-delimiting, so nothing records where the map ends: the map is
 * read first and the padding is whatever is left. That tail has to be exactly
 * the zero bytes that carry the map to its Padmé bucket — a plaintext of any
 * other length was written by something that did not pad as the rule says, and a
 * non-zero byte would make the padding a place to ride bytes past a reader.
 */
function readPaddedPayloadMap(plaintext: Uint8Array): Map<unknown, unknown> {
  const [value, padding] = decodeCborFirst(plaintext, 'malformed_control_payload');
  const expected = paddedLength(BigInt(plaintext.length - padding.length));
  if (BigInt(plaintext.length) !== expected) {
    fail(
      'control_padding_length_mismatch',
      `expected a control-object payload padded to ${expected} bytes, found ${plaintext.length}`,
    );
  }
  if (!isAllZero(padding)) {
    fail('non_zero_control_padding', 'control-object payload padding is not zero-filled');
  }
  return asCborMap(value, 'control_payload_not_a_map', 'a control-object payload');
}

/**
 * Reads one CBOR map, rejecting anything else and anything trailing it.
 *
 * This is for the body the caller hands in, which is the kind's own map alone:
 * the padding is the framing's, and it is added once, around the whole payload.
 */
function readPayloadMap(bytes: Uint8Array): Map<unknown, unknown> {
  const value = decodeCborExact(bytes, 'malformed_control_payload');
  return asCborMap(value, 'control_payload_not_a_map', 'a control-object payload');
}
