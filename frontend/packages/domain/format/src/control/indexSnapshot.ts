/**
 * The payload of an Index Snapshot, ordinary and epoch-activating (FM-16).
 *
 * A Snapshot is the Index of the whole Library at one committed state: the
 * checkpoint it stands at (CK-1, CK-2, CK-3), every current Container, and every
 * current Entry with the Container that holds it. Both Snapshot kinds carry that
 * same content, and the activation kind carries beyond it the two fields that
 * say which head it fenced (MR-2) — so one schema serves both, and which of them
 * an object is stays where FM-11 put it: in the authenticated header.
 *
 * An Entry names its Container by index into `containers` rather than by ID,
 * because a Library holds far more Entries than Containers and the 16-byte ID
 * would otherwise be repeated once per Entry. That index is the one thing a
 * reader has to check beyond the field shapes: an index past the end of
 * `containers` is a Snapshot that cannot be read back into an Index at all.
 *
 * What a Snapshot never carries is device state (CK-7) — including which
 * checkpoint object an Index adopted, which is that Index's own provenance
 * rather than Library content, and has no field here to be written into.
 */

import { comparePaths, compareBytes, requireStrictlyIncreasing } from './canonicalOrder.js';
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
} from '../internal/cbor.js';
import {
  decodeCatalogEntryMap,
  encodeCatalogEntryMap,
} from '../internal/catalogEntryMap.js';
import { fail } from '../errors.js';
import { Generation } from '../model/generation.js';
import { requireKeyringCommitment } from '../model/indexCheckpoint.js';
import type { ContainerSummary } from '../model/containerSummary.js';
import type { EntryLocation } from '../model/entryLocation.js';
import type { ControlObjectKind } from '../model/kinds.js';
import type { SnapshotContent } from '../model/snapshotContent.js';

/** The schema this package writes for an Index Snapshot payload (FM-16). */
export const INDEX_SNAPSHOT_SCHEMA = 1n;

/** What a field of the wrong shape in this schema is reported as. */
const MALFORMED = 'malformed_index_snapshot';

const BASE_HEAD_GENERATION = 'base_head_generation';
const ACTIVATION_SLOT = 'activation_slot';

/**
 * What an activation Index Snapshot carries beyond the checkpoint (MR-2).
 *
 * An activation Snapshot wins a head's commit slot instead of a Journal record,
 * which is what atomically fences the writers still on the old epoch (CP-3).
 * These two fields record that act: which head was fenced, and the slot the
 * fence was won at.
 */
export interface SnapshotActivation {
  /**
   * The generation of the head whose commit slot this activation consumed.
   *
   * It is one less than the Snapshot's own generation, which the header carries
   * (FM-13); it is stated here because the payload has to be able to disagree
   * with the header for a reader to catch a Snapshot that was moved.
   */
  baseHeadGeneration: Generation;
  /**
   * The Storage's own opaque token for that slot, absent where the provider
   * mints none (CP-2, CP-15).
   *
   * A name-keyed Storage persists no token at all, so this being absent says
   * nothing about which kind of Snapshot this is; `baseHeadGeneration` is what
   * must agree with the header's kind.
   */
  activationSlot?: string;
}

/** One Index Snapshot payload: the Library-wide content, activation or not. */
export interface IndexSnapshotPayload {
  /** The Library-wide content this Snapshot holds (CK-7). */
  content: SnapshotContent;
  /** Set on an activation Snapshot, absent on an ordinary one (MR-2). */
  activation?: SnapshotActivation;
}

/**
 * Which control-object kind a payload has to be framed as (FM-11).
 *
 * The two kinds share this schema, so the kind follows from whether the
 * activation fields are there rather than from a flag a caller could set against
 * them.
 */
export function indexSnapshotKind(payload: IndexSnapshotPayload): ControlObjectKind {
  return payload.activation === undefined ? 'index-snapshot' : 'activation-snapshot';
}

/**
 * Serializes an Index Snapshot to the payload a control object carries (FM-16).
 *
 * The epoch comes off the checkpoint, so the payload the framing seals and the
 * content it was made from cannot name two different Master Keys (CK-3, FM-13).
 *
 * Putting `containers` in Container ID order and `entries` in Entry Path order
 * happens here, whatever order the Index that produced this content reported
 * them in.
 */
export function encodeIndexSnapshot(payload: IndexSnapshotPayload): ControlPayload {
  const { checkpoint } = payload.content;
  const containers = [...payload.content.containers].sort((left, right) =>
    compareBytes(left.id.bytes(), right.id.bytes()),
  );
  const positions = new Map(containers.map((container, index) => [container.id.toHex(), index]));
  const entries = [...payload.content.entries].sort((left, right) =>
    comparePaths(left.entry.path, right.entry.path),
  );

  const keyring = requireKeyringCommitment(checkpoint.keyring);

  const map = new Map<string, unknown>([
    ['schema', INDEX_SNAPSHOT_SCHEMA],
    ['head_generation', checkpoint.headGeneration.value],
    ['journal_generation', checkpoint.journalGeneration.value],
  ]);
  if (checkpoint.nextCommitSlot !== undefined) {
    map.set('next_commit_slot', checkpoint.nextCommitSlot);
  }
  map.set('keyring_generation', keyring.generation.value);
  map.set('keyring_replica_count', BigInt(keyring.replicaCount));
  map.set('keyring_set_digest', keyring.setDigest);
  map.set('containers', containers.map(encodeContainerMap));
  map.set(
    'entries',
    entries.map((location, index) => {
      const position = positions.get(location.containerId.toHex());
      if (position === undefined) {
        fail(
          'snapshot_entry_without_container',
          `entry ${index} is held by ${location.containerId.toHex()}, which this Snapshot does not list`,
        );
      }
      const entry = encodeCatalogEntryMap(location.entry);
      entry.set('container', BigInt(position));
      return entry;
    }),
  );

  if (payload.activation !== undefined) {
    map.set(BASE_HEAD_GENERATION, payload.activation.baseHeadGeneration.value);
    if (payload.activation.activationSlot !== undefined) {
      map.set(ACTIVATION_SLOT, payload.activation.activationSlot);
    }
  }

  return {
    masterKeyEpoch: checkpoint.masterKeyEpoch,
    body: encodeCbor(map, 'control_payload_encode_failed'),
  };
}

/**
 * Parses an Index Snapshot out of the payload a control object carried (FM-16).
 *
 * `kind` is what the object's authenticated header declared, and it decides
 * which payload this may be: the activation fields belong to `0x04` alone, so an
 * ordinary Snapshot carrying them and an activation Snapshot without them are
 * both rejected. That is the whole of the cross-check between the header and the
 * payload, and it is why a misfiled Snapshot cannot be read as the other kind.
 *
 * The array orders and every `container` index are verified rather than
 * repaired, for the reason FM-16 gives.
 */
export function decodeIndexSnapshot(
  payload: ControlPayload,
  kind: ControlObjectKind,
): IndexSnapshotPayload {
  if (kind !== 'index-snapshot' && kind !== 'activation-snapshot') {
    fail('not_an_index_snapshot_kind', `a control object of kind ${kind} is no Index Snapshot`);
  }
  const activating = kind === 'activation-snapshot';

  const map = asCborMap(
    decodeCborExact(payload.body, MALFORMED),
    MALFORMED,
    'an Index Snapshot payload',
  );

  const schema = requiredUint(map, 'schema', MALFORMED);
  if (schema < INDEX_SNAPSHOT_SCHEMA) {
    fail('unsupported_index_snapshot_schema', `unsupported Index Snapshot payload schema ${schema}`);
  }

  const containers: ContainerSummary[] = requiredArray(map, 'containers', MALFORMED).map(
    (container, index) =>
      decodeContainerMap(asCborMap(container, MALFORMED, `container ${index}`), MALFORMED),
  );
  requireStrictlyIncreasing('containers', containers, (left, right) =>
    compareBytes(left.id.bytes(), right.id.bytes()),
  );

  const entries: EntryLocation[] = requiredArray(map, 'entries', MALFORMED).map((value, index) => {
    const entry = asCborMap(value, MALFORMED, `entry ${index}`);
    const container = requiredUint(entry, 'container', MALFORMED);
    if (container >= BigInt(containers.length)) {
      fail(
        'dangling_container_index',
        `entry ${index} names container ${container}, not one of the ${containers.length} this Snapshot lists`,
      );
    }
    return {
      containerId: containers[Number(container)].id,
      // `container` is the Snapshot's own field and no part of the entry map,
      // and the entry-map reader ignores what it does not know, so the whole
      // map is handed over as it stands.
      entry: decodeCatalogEntryMap(entry, MALFORMED),
    };
  });
  requireStrictlyIncreasing('entries', entries, (left, right) =>
    comparePaths(left.entry.path, right.entry.path),
  );

  const content: SnapshotContent = {
    checkpoint: {
      masterKeyEpoch: payload.masterKeyEpoch,
      headGeneration: Generation.of(requiredUint(map, 'head_generation', MALFORMED)),
      journalGeneration: Generation.of(requiredUint(map, 'journal_generation', MALFORMED)),
      keyring: requireKeyringCommitment({
        generation: Generation.of(requiredUint(map, 'keyring_generation', MALFORMED)),
        replicaCount: Number(requiredUint(map, 'keyring_replica_count', MALFORMED)),
        setDigest: requiredText(map, 'keyring_set_digest', MALFORMED),
      }),
    },
    containers,
    entries,
  };
  const nextCommitSlot = optionalText(map, 'next_commit_slot', MALFORMED);
  if (nextCommitSlot !== undefined) {
    content.checkpoint.nextCommitSlot = nextCommitSlot;
  }

  if (!activating) {
    for (const field of [BASE_HEAD_GENERATION, ACTIVATION_SLOT]) {
      if (map.get(field) !== undefined) {
        fail(
          'activation_field_on_ordinary_snapshot',
          `an ordinary Index Snapshot carries no ${field}`,
        );
      }
    }
    return { content };
  }

  const baseHeadGeneration = optionalUint(map, BASE_HEAD_GENERATION, MALFORMED);
  if (baseHeadGeneration === undefined) {
    fail(
      'activation_snapshot_field_missing',
      `an activation Index Snapshot carries ${BASE_HEAD_GENERATION}`,
    );
  }
  const activation: SnapshotActivation = {
    baseHeadGeneration: Generation.of(baseHeadGeneration),
  };
  const activationSlot = optionalText(map, ACTIVATION_SLOT, MALFORMED);
  if (activationSlot !== undefined) {
    activation.activationSlot = activationSlot;
  }
  return { content, activation };
}
