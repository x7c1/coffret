/**
 * The payload of a Keyring replica (FM-17).
 *
 * A Keyring generation records what the committed control state holds for every
 * current Container: its Key Envelope (FM-14), or the explicit key-lost marker
 * saying no envelope is reachable (KL-7). That mapping is the whole of the
 * payload — every replica of one generation carries it identically, which is why
 * reading needs one valid replica and the replica count buys redundancy rather
 * than a quorum (KL-6).
 *
 * Three things a replica states are not in the map, because the framing already
 * carries them and one state must not have two answers: the Keyring's generation
 * and the replica position are the control-object header's (FM-11), and
 * `master_key_epoch` is the payload field FM-13 gives every kind. So
 * {@link encodeKeyring} is handed the epoch to seal the mapping under, and two
 * replicas of one generation differ only in their header and their nonce.
 *
 * {@link keyringSetDigest} is the fourth thing kept out of the map, and the only
 * one kept out for a reason of its own: the digest is taken *over* the mapping,
 * so a field carrying it would make it cover itself. It lives here beside the
 * encoder because the bytes it hashes are the encoder's own `mapping` array —
 * the name a replica is stored under (FM-12), the commitment a commit selects
 * (CP-10, KL-3), and KL-1's validity check all read that one definition.
 *
 * Putting `mapping` in Container ID order is the encoder's job and checking it
 * is the decoder's — see `canonicalOrder` for why a reader rejects a payload
 * rather than sorting it.
 */

import { blake3 } from '@noble/hashes/blake3.js';

import { compareBytes, requireStrictlyIncreasing } from './canonicalOrder.js';
import type { ControlPayload } from './payload.js';
import {
  asCborMap,
  decodeCborExact,
  encodeCbor,
  optionalBool,
  requiredArray,
  requiredBytes,
  requiredUint,
  type CborMap,
} from '../internal/cbor.js';
import { toHex } from '../internal/bytes.js';
import { fail } from '../errors.js';
import { ContainerId } from '../model/containerId.js';
import { KeyEnvelope } from '../model/keyEnvelope.js';
import type { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import type { KeyringEntry, KeyringMapping } from '../model/keyringMapping.js';

/** The schema this package writes for a Keyring payload (FM-17). */
export const KEYRING_SCHEMA = 1n;

/** What a field of the wrong shape in this schema is reported as. */
const MALFORMED = 'malformed_keyring_payload';

const MAPPING = 'mapping';
const ID = 'id';
const ENVELOPE = 'envelope';
const KEY_LOST = 'key_lost';

/**
 * Serializes a Keyring mapping to the payload a replica carries (FM-17).
 *
 * The epoch is handed in rather than taken off the mapping: which epoch a
 * generation belongs to is the Keyring's own numbering (KL-10) and not something
 * the mapping states, so the caller that knows which Master Key is sealing this
 * replica names it once, here (FM-13).
 *
 * Putting `mapping` in Container ID order happens here, whatever order the
 * caller held the entries in.
 */
export function encodeKeyring(
  mapping: KeyringMapping,
  masterKeyEpoch: MasterKeyEpoch,
): ControlPayload {
  const map = new Map<string, unknown>([
    ['schema', KEYRING_SCHEMA],
    [MAPPING, mappingValue(mapping)],
  ]);
  return { masterKeyEpoch, body: encodeCbor(map, 'control_payload_encode_failed') };
}

/**
 * The digest binding one Keyring generation's mapping (FM-17).
 *
 * It is the BLAKE3-256 of the `mapping` array alone — the array exactly as the
 * payload carries it, in Container ID order — and it is deliberately not a field
 * of that payload: a digest carried inside the thing it covers would have to
 * cover itself.
 *
 * One definition therefore serves three readers. A replica's object name carries
 * this value (FM-12), a commit selects a replica set by it (CP-10, KL-3), and a
 * reader recomputes it from a decoded mapping to decide whether the replica it
 * fetched is the one that name promised (KL-1).
 *
 * The result is the lowercase hex text those three carry it in, not the raw 32
 * bytes: the name grammar spells it that way, and one digest with one spelling
 * is what keeps a commitment comparable as it travels.
 */
export function keyringSetDigest(mapping: KeyringMapping): string {
  return toHex(blake3(keyringDigestInput(mapping)));
}

/**
 * The bytes the digest is taken over: the `mapping` array, encoded.
 *
 * Named apart from the hash so that what FM-17 makes normative has somewhere to
 * be examined. Everything else this package writes is one valid CBOR spelling
 * among several — a reader takes any of them — while these bytes are the
 * spelling, because a second one would be a second digest for one mapping.
 */
export function keyringDigestInput(mapping: KeyringMapping): Uint8Array {
  return encodeCbor(mappingValue(mapping), 'control_payload_encode_failed');
}

/**
 * Parses a Keyring mapping out of the payload a replica carried (FM-17).
 *
 * What the mapping says about the Library — that it covers every current
 * Container and no other (KL-7) — needs the Journal to check and is no part of
 * reading one replica. What is checked here is what makes these bytes a mapping
 * at all: every element maps its Container to exactly one thing, and the
 * elements are in the order that gives one mapping one `set_digest`.
 *
 * The digest itself is not checked here either, because the payload does not
 * carry it: a caller compares {@link keyringSetDigest} of what this returns
 * against the name it fetched the replica under (FM-12, KL-1).
 */
export function decodeKeyring(payload: ControlPayload): KeyringMapping {
  const map = asCborMap(
    decodeCborExact(payload.body, MALFORMED),
    MALFORMED,
    'a Keyring payload',
  );

  const schema = requiredUint(map, 'schema', MALFORMED);
  if (schema < KEYRING_SCHEMA) {
    fail('unsupported_keyring_schema', `unsupported Keyring payload schema ${schema}`);
  }

  const entries = requiredArray(map, MAPPING, MALFORMED).map((element, index) =>
    decodeEntry(asCborMap(element, MALFORMED, `element ${index} of ${MAPPING}`), index),
  );
  requireStrictlyIncreasing(MAPPING, entries, (left, right) =>
    compareBytes(left.containerId.bytes(), right.containerId.bytes()),
  );

  return { entries };
}

/**
 * The `mapping` array, in the Container ID order FM-17 fixes.
 *
 * {@link keyringSetDigest} hashes exactly this value's encoding, so this is the
 * one array in the package whose bytes are normative rather than one valid CBOR
 * spelling among several. What that costs is stated in {@link encodeEntry}; what
 * it buys is that one mapping has one digest whichever device wrote it (KL-1,
 * KL-14).
 */
function mappingValue(mapping: KeyringMapping): Map<string, unknown>[] {
  return [...mapping.entries]
    .sort((left, right) => compareBytes(left.containerId.bytes(), right.containerId.bytes()))
    .map(encodeEntry);
}

/**
 * One element: the Container's ID, then the one thing the Keyring holds for it.
 *
 * The two fields are inserted in that order deliberately. FM-17 hashes this
 * array as deterministic CBOR, whose map keys are ordered by their encoded
 * bytes: `id` is two characters and both `envelope` and `key_lost` are eight, so
 * `id` comes first. A writer that emitted them the other way round would produce
 * a payload every reader still accepts — the maps are read by name — and a
 * `set_digest` no other implementation computes.
 */
function encodeEntry(entry: KeyringEntry): Map<string, unknown> {
  const map = new Map<string, unknown>([[ID, entry.containerId.bytes()]]);
  if (entry.key.status === 'envelope') {
    map.set(ENVELOPE, entry.key.envelope.bytes());
  } else {
    // The marker's presence is what records the loss; FM-17 spells it `true` so
    // that one marker has one spelling.
    map.set(KEY_LOST, true);
  }
  return map;
}

/** One element read back: a Container, and the one thing the Keyring holds. */
function decodeEntry(map: CborMap, index: number): KeyringEntry {
  const containerId = ContainerId.fromBytes(requiredBytes(map, ID, MALFORMED));

  const envelope = map.get(ENVELOPE) === undefined
    ? undefined
    : KeyEnvelope.fromBytes(requiredBytes(map, ENVELOPE, MALFORMED));
  const marker = optionalBool(map, KEY_LOST, MALFORMED);
  // FM-17 spells the marker `true`. A `false` there is not "no marker": it is a
  // writer stating the field in a form the rule does not define, and reading it
  // as an absence would put two spellings of an envelope's presence into
  // circulation. It is refused under a code of its own rather than as a
  // malformed payload: the field carried the type the schema gives it, so what
  // a caller learns from this is the same kind of thing the two codes below
  // report, not that the CBOR was unreadable.
  if (marker === false) {
    fail(
      'keyring_entry_marker_not_true',
      `element ${index} of ${MAPPING} spells its key-lost marker false rather than true`,
    );
  }

  if (envelope !== undefined && marker !== undefined) {
    fail(
      'keyring_entry_with_envelope_and_marker',
      `element ${index} of ${MAPPING} carries a Key Envelope and a key-lost marker at once`,
    );
  }
  if (envelope === undefined && marker === undefined) {
    fail(
      'keyring_entry_without_envelope_or_marker',
      `element ${index} of ${MAPPING} carries neither a Key Envelope nor a key-lost marker`,
    );
  }
  return {
    containerId,
    key: envelope === undefined ? { status: 'key-lost' } : { status: 'envelope', envelope },
  };
}
