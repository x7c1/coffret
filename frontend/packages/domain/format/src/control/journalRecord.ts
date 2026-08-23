/**
 * The payload of a Journal record (FM-15).
 *
 * A record is the commit point of a batch (CP-1), and its payload is the whole
 * of what a device needs to replay that commit without opening a Container: the
 * Keyring tuple the commit selected (CP-10), the two slots the head reserves
 * (CP-2, CK-10), the Containers the batch added with their entry tables
 * (CP-11), and the Container IDs it removed (CP-14).
 *
 * Two of the record's fields are not in the map, because the framing already
 * carries them and one state must not have two answers: the record's own
 * generation is the control-object header's (FM-11), and `master_key_epoch` is
 * the payload field FM-13 gives every kind. So the encoder hands back a whole
 * {@link ControlPayload}, and the decoder is told the generation the header
 * carried.
 *
 * Putting `additions` and `removals` in Container ID order is the encoder's job
 * and checking that order is the decoder's — see `canonicalOrder` for why a
 * reader rejects a payload rather than sorting it.
 */

import { compareBytes, requireStrictlyIncreasing } from './canonicalOrder.js';
import { decodeContainerMap, encodeContainerMap } from './wireContainer.js';
import type { ControlPayload } from './payload.js';
import {
  asCborMap,
  decodeCborExact,
  encodeCbor,
  optionalText,
  optionalUint,
  requiredArray,
  requiredText,
  requiredUint,
  type CborMap,
} from '../internal/cbor.js';
import { decodeEntryMap, encodeEntryMap } from '../internal/entryMap.js';
import { takeExactly } from '../internal/bytes.js';
import { fail } from '../errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from '../model/containerId.js';
import { Generation } from '../model/generation.js';
import { requireKeyringCommitment } from '../model/indexCheckpoint.js';
import type { ContainerAddition, JournalRecord } from '../model/journalRecord.js';

/** The schema this package writes for a Journal record payload (FM-15). */
export const JOURNAL_RECORD_SCHEMA = 1n;

/** What a field of the wrong shape in this schema is reported as. */
const MALFORMED = 'malformed_journal_record';

/**
 * Serializes a Journal record to the payload a control object carries (FM-15).
 *
 * The epoch comes off the record itself, so the payload the framing seals and
 * the record it was made from cannot name two different Master Keys (FM-13).
 */
export function encodeJournalRecord(record: JournalRecord): ControlPayload {
  const additions = [...record.additions].sort((left, right) =>
    compareBytes(left.container.id.bytes(), right.container.id.bytes()),
  );
  const removals = [...record.removals].sort((left, right) =>
    compareBytes(left.bytes(), right.bytes()),
  );
  const keyring = requireKeyringCommitment(record.keyring);

  const map = new Map<string, unknown>([['schema', JOURNAL_RECORD_SCHEMA]]);
  if (record.prev !== undefined) {
    map.set('prev', record.prev.value);
  }
  if (record.nextCommitSlot !== undefined) {
    map.set('next_commit_slot', record.nextCommitSlot);
  }
  if (record.snapshotSlot !== undefined) {
    map.set('snapshot_slot', record.snapshotSlot);
  }
  map.set('keyring_generation', keyring.generation.value);
  map.set('keyring_replica_count', BigInt(keyring.replicaCount));
  map.set('keyring_set_digest', keyring.setDigest);
  map.set('additions', additions.map(encodeAddition));
  map.set(
    'removals',
    removals.map((id) => id.bytes()),
  );

  return {
    masterKeyEpoch: record.masterKeyEpoch,
    body: encodeCbor(map, 'control_payload_encode_failed'),
  };
}

/**
 * One addition: the Container's five fields, then its entry table (CP-11).
 *
 * The entry table keeps the order the Container's own meta section gives it,
 * which is the plaintext stream order FM-9 fixes — the record carries a copy of
 * that table, so re-ordering it here would make the copy disagree with the
 * original.
 */
function encodeAddition(addition: ContainerAddition): Map<string, unknown> {
  const map = encodeContainerMap(addition.container);
  map.set('entries', addition.entries.map(encodeEntryMap));
  return map;
}

/**
 * Parses a Journal record out of the payload a control object carried (FM-15).
 *
 * The generation is the one the object's own header declared: a record does not
 * repeat it, so the caller passes what the framing authenticated (FM-11).
 *
 * `prev` is the record's own statement of the head it was built on, and it is
 * held against that authenticated generation here, so a replay follows the chain
 * out of the payload rather than out of the name the object was fetched under
 * (FM-15).
 *
 * The array orders are verified rather than restored, for the reason FM-15
 * gives.
 */
export function decodeJournalRecord(
  payload: ControlPayload,
  generation: Generation,
): JournalRecord {
  const map = asCborMap(
    decodeCborExact(payload.body, MALFORMED),
    MALFORMED,
    'a Journal record payload',
  );

  const schema = requiredUint(map, 'schema', MALFORMED);
  if (schema < JOURNAL_RECORD_SCHEMA) {
    fail('unsupported_journal_record_schema', `unsupported Journal record payload schema ${schema}`);
  }

  const additions = requiredArray(map, 'additions', MALFORMED).map((addition, index) =>
    decodeAddition(asCborMap(addition, MALFORMED, `addition ${index}`)),
  );
  requireStrictlyIncreasing('additions', additions, (left, right) =>
    compareBytes(left.container.id.bytes(), right.container.id.bytes()),
  );

  const removals = requiredArray(map, 'removals', MALFORMED).map((removal) => {
    if (!(removal instanceof Uint8Array)) {
      fail(MALFORMED, 'a removal is a byte string');
    }
    return ContainerId.fromBytes(takeExactly(removal, CONTAINER_ID_LENGTH, 'a Container ID'));
  });
  requireStrictlyIncreasing('removals', removals, (left, right) =>
    compareBytes(left.bytes(), right.bytes()),
  );

  const record: JournalRecord = {
    generation,
    masterKeyEpoch: payload.masterKeyEpoch,
    keyring: requireKeyringCommitment({
      generation: Generation.of(requiredUint(map, 'keyring_generation', MALFORMED)),
      replicaCount: Number(requiredUint(map, 'keyring_replica_count', MALFORMED)),
      setDigest: requiredText(map, 'keyring_set_digest', MALFORMED),
    }),
    additions,
    removals,
  };

  const prev = optionalUint(map, 'prev', MALFORMED);
  requirePrev(generation, prev);
  if (prev !== undefined) {
    record.prev = Generation.of(prev);
  }
  const nextCommitSlot = optionalText(map, 'next_commit_slot', MALFORMED);
  if (nextCommitSlot !== undefined) {
    record.nextCommitSlot = nextCommitSlot;
  }
  const snapshotSlot = optionalText(map, 'snapshot_slot', MALFORMED);
  if (snapshotSlot !== undefined) {
    record.snapshotSlot = snapshotSlot;
  }
  return record;
}

/**
 * Holds `prev` to the generation the framing authenticated (FM-15).
 *
 * A record at generation *g* succeeds head *g − 1*, so its statement of what it
 * was built on has exactly one right value; the Library's first head was built
 * on nothing, so it is the one record that states no predecessor (FM-13).
 */
function requirePrev(generation: Generation, prev: bigint | undefined): void {
  const expected = generation.value === 0n ? undefined : generation.value - 1n;
  if (prev === expected) {
    return;
  }
  fail(
    'journal_record_prev_mismatch',
    prev === undefined
      ? `the Journal record at generation ${generation} states no head it succeeds`
      : `the Journal record at generation ${generation} states ${prev} as the head it succeeds`,
  );
}

function decodeAddition(map: CborMap): ContainerAddition {
  return {
    container: decodeContainerMap(map, MALFORMED),
    entries: requiredArray(map, 'entries', MALFORMED).map((entry, index) =>
      decodeEntryMap(asCborMap(entry, MALFORMED, `entry ${index}`), MALFORMED),
    ),
  };
}
