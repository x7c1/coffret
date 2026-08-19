/**
 * What a control object carries inside its AEAD message.
 *
 * The payload is one CBOR map. This module owns exactly one of its fields —
 * `master_key_epoch`, which every control object carries whatever its kind
 * (FM-13) — and treats the rest as the kind's own business: the caller hands
 * over the CBOR map of its fields, and gets those fields back on the way out.
 * The bytes they come back as need not be the bytes they went in as, for the
 * reason [`ControlPayload.body`] gives.
 */

import { asCborMap, decodeCborExact, encodeCbor, asUint } from '../internal/cbor.js';
import { fail } from '../errors.js';
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

/** Serializes a payload: the kind's own map with `master_key_epoch` added. */
export function encodeControlPayload(payload: ControlPayload): Uint8Array {
  const map = readPayloadMap(payload.body);
  if (map.has(MASTER_KEY_EPOCH)) {
    fail('malformed_control_payload', `the body already carries ${MASTER_KEY_EPOCH}`);
  }
  map.set(MASTER_KEY_EPOCH, payload.masterKeyEpoch.value);
  return encodeCbor(map, 'control_payload_encode_failed');
}

/** Parses a payload, insisting that it says which epoch encrypted it. */
export function decodeControlPayload(bytes: Uint8Array): ControlPayload {
  const map = readPayloadMap(bytes);
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

/** Reads one CBOR map, rejecting anything else and anything trailing it. */
function readPayloadMap(bytes: Uint8Array): Map<unknown, unknown> {
  const value = decodeCborExact(bytes, 'malformed_control_payload');
  return asCborMap(value, 'control_payload_not_a_map', 'a control-object payload');
}
