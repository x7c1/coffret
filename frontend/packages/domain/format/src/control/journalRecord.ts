/**
 * The payload of a Journal record (spec: FM-15).
 *
 * A record is the commit point of a batch (spec: CP-1), and its payload is the
 * whole of what a device needs to replay that commit without opening a
 * Container: the Keyring tuple the commit selected (spec: CP-10), the two slots
 * the head reserves (spec: CP-2, CK-10), the Containers the batch added
 * with their entry tables (spec: CP-11), the Container IDs it removed
 * (spec: CP-14), and the Entries it gave a new current Entry Path, each named
 * by its Container and entry number (spec: FM-15).
 *
 * Two of the record's fields are not in the map, because the framing already
 * carries them and one state must not have two answers: the record's own
 * generation is the control-object header's (spec: FM-11), and
 * `master_key_epoch` is the payload field FM-13 gives every kind. So the
 * encoder hands back a whole {@link ControlPayload}, and the decoder is told
 * the generation the header carried.
 *
 * Putting `additions` and `removals` in Container ID order, and `renames` in
 * Container ID and then entry number order, is the encoder's job and checking
 * those orders is the decoder's — see `canonicalOrder` for why a
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
  requiredBytes,
  requiredText,
  requiredUint,
  type CborMap,
} from '../internal/cbor.js';
import {
  decodeCatalogEntryMap,
  encodeCatalogEntryMap,
} from '../internal/catalogEntryMap.js';
import { takeExactly } from '../internal/bytes.js';
import { fail } from '../errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from '../model/containerId.js';
import { MAX_ENTRY_NUMBER, isDerivedPath, type EntryRename } from '../model/entryRename.js';
import { storedPath } from '../internal/entryFields.js';
import { Generation } from '../model/generation.js';
import { requireKeyringCommitment } from '../model/indexCheckpoint.js';
import type { ContainerAddition, JournalRecord } from '../model/journalRecord.js';
import { requireLibraryName } from '../model/libraryName.js';
import { JOURNAL_RECORD, fromWire, toWire } from '../internal/fieldNumbers.js';

/**
 * The schema this package writes for a Journal record payload, and the one it
 * implements (spec: FM-15): 5, the first whose records may carry
 * `library_name`.
 *
 * Not what a reader refuses below: that is
 * {@link JOURNAL_RECORD_LOWEST_READ_SCHEMA}, so that a later build writing a new
 * field under a higher schema goes on reading the records written before it.
 * Nor what every record names in `required_schema`: that is
 * {@link JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN}.
 */
export const JOURNAL_RECORD_SCHEMA = 5n;

/**
 * What every record names in `required_schema`: 4 (spec: FM-9, FM-15).
 *
 * A reader that ignored `deposited_at` would hold a catalog without deposit
 * times and could commit records no reader of schema 4 accepts, so that field
 * is must-understand and every record says so. `library_name`, which schema 5
 * adds, is not: a reader that ignores it loses the Library's name and nothing
 * else, so the value stays 4.
 */
export const JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN = 4n;

/**
 * The lowest schema a reader accepts, in `schema` and in `required_schema`
 * alike: 4, the first whose addition entries carry `deposited_at`
 * (spec: FM-15).
 *
 * A record of an earlier schema carries Entries without a deposit time, so it is
 * refused as a schema this package does not read rather than read with a time
 * made up. A record of schema 4 is one written before records could carry a
 * name, and reads as one that carries none.
 */
export const JOURNAL_RECORD_LOWEST_READ_SCHEMA = 4n;

/** What a field of the wrong shape in this schema is reported as. */
const MALFORMED = 'malformed_journal_record';

/**
 * Serializes a Journal record to the payload a control object carries
 * (spec: FM-15).
 *
 * The epoch comes off the record itself, so the payload the framing seals and
 * the record it was made from cannot name two different Master Keys
 * (spec: FM-13).
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
  // Left out where the record renames nothing, so a record without renames
  // costs nothing for them (spec: FM-15).
  if (record.renames.length > 0) {
    map.set('renames', [...record.renames].sort(compareRenames).map(encodeRename));
  }
  // Every record names the schema its deposit times need, so a reader of an
  // earlier schema stops at it rather than holding a catalog without them
  // (spec: FM-9, FM-15).
  map.set('required_schema', JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN);
  // Left out where the record gives the Library no name (spec: FM-15).
  if (record.libraryName !== undefined) {
    map.set('library_name', requireLibraryName(record.libraryName, 'control_payload_encode_failed'));
  }

  return {
    masterKeyEpoch: record.masterKeyEpoch,
    body: encodeCbor(
      toWire(map, JOURNAL_RECORD, 'control_payload_encode_failed'),
      'control_payload_encode_failed',
    ),
  };
}

/**
 * One addition: the Container's five fields, then its entry table
 * (spec: CP-11).
 *
 * The entry table keeps the order the Container's own meta section gives it,
 * which is the plaintext stream order FM-9 fixes — the record carries a copy of
 * that table, so re-ordering it here would make the copy disagree with the
 * original. Each Entry carries its deposit time, which no meta section holds
 * (spec: FM-15).
 */
function encodeAddition(addition: ContainerAddition): Map<string, unknown> {
  if (addition.depositTimes.length !== addition.entries.length) {
    fail(
      'control_payload_encode_failed',
      `an addition of ${addition.entries.length} Entries carries ${addition.depositTimes.length} deposit times`,
    );
  }
  const map = encodeContainerMap(addition.container);
  map.set(
    'entries',
    addition.entries.map((entry, index) =>
      encodeCatalogEntryMap(entry, addition.depositTimes[index] as bigint),
    ),
  );
  return map;
}

/** The order FM-15 gives `renames`: Container ID bytes, then entry number. */
function compareRenames(left: EntryRename, right: EntryRename): number {
  return (
    compareBytes(left.containerId.bytes(), right.containerId.bytes()) ||
    left.entryNumber - right.entryNumber
  );
}

/** One rename: the Container, the entry number, and the new path. */
function encodeRename(rename: EntryRename): Map<string, unknown> {
  return new Map<string, unknown>([
    ['container', rename.containerId.bytes()],
    ['entry', BigInt(rename.entryNumber)],
    ['path', rename.path],
  ]);
}

/**
 * Parses a Journal record out of the payload a control object carried
 * (spec: FM-15).
 *
 * The generation is the one the object's own header declared: a record does not
 * repeat it, so the caller passes what the framing authenticated (spec: FM-11).
 *
 * `prev` is the record's own statement of the head it was built on, and it is
 * held against that authenticated generation here, so a replay follows the chain
 * out of the payload rather than out of the name the object was fetched under
 * (spec: FM-15).
 *
 * The array orders are verified rather than restored, for the reason FM-15
 * gives. Of the rules FM-15 gives `renames`, this holds the ones the record
 * alone shows — the order, a path that is an Entry Path outside the reserved
 * derived namespace — and leaves the ones that need the catalog to whoever
 * applies the record.
 *
 * `required_schema` is read before anything else the record holds, and a record
 * naming a schema above the one this package implements is refused there as
 * `journal_record_needs_newer_build`: never skipped, and never applied in part
 * with the fields this package does not know ignored (spec: FM-9, FM-15).
 */
export function decodeJournalRecord(
  payload: ControlPayload,
  generation: Generation,
): JournalRecord {
  const map = asCborMap(
    fromWire(decodeCborExact(payload.body, MALFORMED), JOURNAL_RECORD),
    MALFORMED,
    'a Journal record payload',
  );

  const schema = requiredUint(map, 'schema', MALFORMED);
  if (schema < JOURNAL_RECORD_LOWEST_READ_SCHEMA) {
    fail('unsupported_journal_record_schema', `unsupported Journal record payload schema ${schema}`);
  }
  requireSchemaNamed(map, generation);

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

  // Absent on a record that renames nothing (spec: FM-15).
  const renames = map.has('renames')
    ? requiredArray(map, 'renames', MALFORMED).map((rename, index) =>
        decodeRename(asCborMap(rename, MALFORMED, `rename ${index}`), index),
      )
    : [];
  requireStrictlyIncreasing('renames', renames, compareRenames);

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
    renames,
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
  // Absent on a record that names nothing, which every record of schema 4 is
  // (spec: FM-15).
  const libraryName = optionalText(map, 'library_name', MALFORMED);
  if (libraryName !== undefined) {
    record.libraryName = requireLibraryName(libraryName, MALFORMED);
  }
  return record;
}

/**
 * The schema the record says its reader must implement, held against the one
 * this package does (spec: FM-9, FM-15).
 *
 * Above it, the record needs a newer build. Below the lowest schema a reader
 * accepts, or absent, the record is malformed: every record a reader accepts
 * names one at least that high, so a lower value is no state a writer produces.
 */
function requireSchemaNamed(map: CborMap, generation: Generation): void {
  const required = optionalUint(map, 'required_schema', MALFORMED);
  if (required !== undefined && required > JOURNAL_RECORD_SCHEMA) {
    fail(
      'journal_record_needs_newer_build',
      `the Journal record at generation ${generation} needs a reader of schema ${required}, ` +
        `and this build implements schema ${JOURNAL_RECORD_SCHEMA}; a newer build of coffret ` +
        'is needed to read the Library past it',
    );
  }
  if (required === undefined || required < JOURNAL_RECORD_LOWEST_READ_SCHEMA) {
    fail(
      MALFORMED,
      `a record names ${JOURNAL_RECORD_LOWEST_READ_SCHEMA} or above as its required_schema, found ${required ?? 'none'}`,
    );
  }
}

/**
 * Holds `prev` to the generation the framing authenticated (spec: FM-15).
 *
 * A record at generation *g* succeeds head *g − 1*, so its statement of what it
 * was built on has exactly one right value; the Library's first head was built
 * on nothing, so it is the one record that states no predecessor (spec: FM-13).
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

/** One element of `renames`, the `index`-th (spec: FM-15). */
function decodeRename(map: CborMap, index: number): EntryRename {
  const container = requiredBytes(map, 'container', MALFORMED);
  const entry = requiredUint(map, 'entry', MALFORMED);
  if (entry > BigInt(MAX_ENTRY_NUMBER)) {
    fail(MALFORMED, `entry is an entry number below 2^32, found ${entry}`);
  }
  const path = storedPath(map, 'path', MALFORMED, 'path');
  if (isDerivedPath(path)) {
    fail(
      'rename_into_reserved_namespace',
      `element ${index} of renames gives an Entry a path under the component reserved for derived Entries`,
    );
  }
  return {
    containerId: ContainerId.fromBytes(takeExactly(container, CONTAINER_ID_LENGTH, 'a Container ID')),
    entryNumber: Number(entry),
    path,
  };
}

function decodeAddition(map: CborMap): ContainerAddition {
  const decoded = requiredArray(map, 'entries', MALFORMED).map((entry, index) =>
    decodeCatalogEntryMap(asCborMap(entry, MALFORMED, `entry ${index}`), MALFORMED),
  );
  return {
    container: decodeContainerMap(map, MALFORMED),
    entries: decoded.map(({ entry }) => entry),
    depositTimes: decoded.map(({ depositedAtSeconds }) => depositedAtSeconds),
  };
}
