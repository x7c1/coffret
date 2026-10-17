/**
 * Test support: the values the payload schema cases are built out of.
 *
 * They are deliberately dull — a Container ID is one byte repeated and a hash is
 * another — because none of what the cases assert turns on what is in them. What
 * matters is that they are distinguishable in a failure message and that they
 * are handed over *out* of the canonical order, so an encoder that left the
 * order alone would be caught (spec: FM-15, FM-16, FM-17).
 *
 * Excluded from the package build — nothing here ships.
 */

import { decodeCborExact, encodeCbor } from '../internal/cbor.js';
import { fromWire, type MapFields } from '../internal/fieldNumbers.js';
import { toWireLenient } from '../internal/fieldNumbers.testing.js';
import { ContainerId } from '../model/containerId.js';
import { Generation } from '../model/generation.js';
import { KEY_ENVELOPE_LENGTH, KeyEnvelope } from '../model/keyEnvelope.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import type { ContainerSummary } from '../model/containerSummary.js';
import type { EntryLocation } from '../model/entryLocation.js';
import type { EntryMetadata } from '../model/entry.js';
import type { IndexCheckpoint, KeyringCommitment } from '../model/indexCheckpoint.js';
import type { ContainerAddition, JournalRecord } from '../model/journalRecord.js';
import type { KeyTable } from '../model/keyTable.js';
import type { ContainerKind } from '../model/kinds.js';
import type { SnapshotContent } from '../model/snapshotContent.js';
import type { ControlPayload } from './payload.js';
import type { IndexSnapshotPayload } from './indexSnapshot.js';

/** The epoch every payload these helpers build was written under. */
export const EPOCH = MasterKeyEpoch.of(2n);

/** The head every payload these helpers build sits at. */
export const GENERATION = Generation.of(7n);

/** A Container ID whose sixteen bytes are all `seed`. */
export function containerId(seed: number): ContainerId {
  return ContainerId.fromBytes(new Uint8Array(16).fill(seed));
}

/** A content hash whose thirty-two bytes are all `seed`. */
export function contentHash(seed: number): Uint8Array {
  return new Uint8Array(32).fill(seed);
}

/**
 * What a payload records about one Container.
 *
 * An even seed caches the provider's handle for the object and an odd one does
 * not, so a set of two covers both spellings of the optional field
 * (spec: CP-11).
 */
export function summary(seed: number, kind: ContainerKind): ContainerSummary {
  const container: ContainerSummary = {
    id: containerId(seed),
    kind,
    ciphertextHash: contentHash(seed),
    ciphertextLength: BigInt(4096 + seed),
  };
  if (seed % 2 === 0) {
    container.objectRef = `stored-${seed}`;
  }
  return container;
}

/** One entry-table element, laid at `offset` and `size` bytes long (spec: FM-9). */
export function entry(path: string, offset: bigint, size: bigint): EntryMetadata {
  return {
    path,
    capturedPath: path,
    offset,
    size,
    mtimeSeconds: 1_700_000_000n,
    hash: contentHash(0x5b),
  };
}

/** The Keyring commitment a commit at `generation` selects (spec: KL-3). */
export function keyring(generation: bigint): KeyringCommitment {
  return { generation: Generation.of(generation), replicaCount: 3, setDigest: 'beef' };
}

/** The checkpoint an Index stands at once the head at `generation` is applied. */
export function checkpoint(generation: bigint): IndexCheckpoint {
  return {
    masterKeyEpoch: EPOCH,
    headGeneration: Generation.of(generation),
    journalGeneration: Generation.of(generation),
    nextCommitSlot: `minted-${generation}`,
    keyring: keyring(generation),
  };
}

/**
 * The folder the first Entry of every table was written into, before whatever
 * moved it to the path it holds now.
 */
export const CAPTURED_DIR = 'imports';

/** The birth time the one Entry that has one was created at. */
export const BORN = 1_600_000_000n;

/**
 * One Container a record adds, with an entry table laid end to end
 * (spec: FM-4).
 *
 * A Pack's table carries one Entry whose file had a birth time when the
 * Container was written and one whose file had none, so both spellings of the
 * optional field travel (spec: FM-15). The first Entry of every table carries
 * a captured path apart from its current one, as a renamed Entry would, so a
 * codec that wrote one name as the other is caught (spec: FM-15).
 */
export function addition(seed: number, kind: ContainerKind): ContainerAddition {
  const label = seed.toString(16).padStart(2, '0');
  const entries = [
    {
      ...entry(`albums/${label}/cover.jpg`, 0n, 120n),
      capturedPath: `${CAPTURED_DIR}/${label}/cover.jpg`,
    },
  ];
  if (kind === 'pack') {
    entries.push({
      ...entry(`albums/${label}/.thumbs/cover.jpg`, 120n, 40n),
      btimeSeconds: BORN,
      mime: 'image/webp',
      derivedFrom: { containerId: containerId(seed), path: `albums/${label}/cover.jpg` },
    });
  }
  return { container: summary(seed, kind), entries };
}

/**
 * A record with everything a record can carry.
 *
 * The additions and the removals are handed over in the reverse of Container ID
 * order, so a case comparing bytes is comparing what the encoder ordered rather
 * than what a caller happened to hold (spec: FM-15).
 */
export function record(): JournalRecord {
  return {
    generation: GENERATION,
    prev: Generation.of(GENERATION.value - 1n),
    masterKeyEpoch: EPOCH,
    keyring: keyring(4n),
    nextCommitSlot: 'minted-head-8',
    snapshotSlot: 'minted-idx-7',
    additions: [addition(0x40, 'pack'), addition(0x21, 'one-file')],
    removals: [containerId(0x99), containerId(0x11)],
  };
}

/**
 * The Library's first record: nothing before it, and no slot to persist.
 *
 * A name-keyed Storage mints no identifier, so both slots are absent here
 * (spec: CP-2, CP-15) — and generation 0 has no predecessor to state
 * (spec: FM-13).
 */
export function firstRecord(): JournalRecord {
  return {
    generation: Generation.FIRST,
    masterKeyEpoch: EPOCH,
    keyring: keyring(0n),
    additions: [addition(0x40, 'pack')],
    removals: [],
  };
}

/** A Key Envelope whose seventy-two bytes are all `seed`. */
export function envelope(seed: number): KeyEnvelope {
  return KeyEnvelope.fromBytes(new Uint8Array(KEY_ENVELOPE_LENGTH).fill(seed));
}

/**
 * A Keyring's key table holding both of the things a Keyring can hold.
 *
 * Two Containers open through an envelope and one is recorded key-lost
 * (spec: KL-7), and the elements are handed over out of Container ID order on
 * purpose: a case comparing bytes is then comparing what the encoder ordered
 * rather than what a caller happened to hold (spec: FM-17).
 */
export function keyTable(): KeyTable {
  return {
    elements: [
      { containerId: containerId(0x40), key: { status: 'envelope', envelope: envelope(0x40) } },
      { containerId: containerId(0x99), key: { status: 'key-lost' } },
      { containerId: containerId(0x21), key: { status: 'envelope', envelope: envelope(0x21) } },
    ],
  };
}

/**
 * The key table whose digest both implementations pin.
 *
 * Deliberately smaller and duller than {@link keyTable}: it exists so that the
 * two implementations state one expected digest each, in a shape that is easy to
 * spell identically in both languages. The Rust suite builds the same two
 * elements — `11…` with an envelope of `22` bytes, `33…` key-lost — and asserts
 * the same hex.
 */
export function pinnedKeyTable(): KeyTable {
  return {
    elements: [
      { containerId: containerId(0x11), key: { status: 'envelope', envelope: envelope(0x22) } },
      { containerId: containerId(0x33), key: { status: 'key-lost' } },
    ],
  };
}

/**
 * The Entry Path the sample Snapshot's first Entry was written at, before
 * whatever moved it to the path it holds now.
 */
export const CAPTURED_PATH = 'imports/a.jpg';

/**
 * A Library of three Containers whose Entries interleave across them.
 *
 * Interleaving is the point: `entries` is in Entry Path order across the whole
 * Library (spec: EP-3), not grouped by Container, so a case comparing the
 * encoded order to the order the content was handed over in has something to
 * catch.
 *
 * One Entry's file had a birth time when its Container was written and the rest
 * had none, so both spellings of the optional field travel (spec: FM-16). The
 * same Entry carries a captured path apart from its current one, as a renamed
 * Entry would, and the rest carry their current one.
 */
export function content(): SnapshotContent {
  const born = located(0x40, 'albums/spring/a.jpg', 0n, 100n);
  born.entry.btimeSeconds = BORN;
  born.entry.capturedPath = CAPTURED_PATH;
  return {
    checkpoint: checkpoint(GENERATION.value),
    containers: [summary(0x40, 'pack'), summary(0x21, 'one-file'), summary(0x33, 'pack')],
    entries: [
      located(0x33, 'photos/2019/b.jpg', 0n, 90n),
      born,
      located(0x21, 'books/atlas/page-001.png', 0n, 200n),
      located(0x40, 'photos/2019/a.jpg', 100n, 80n),
    ],
  };
}

/** The ordinary checkpoint of that head (spec: CK-10). */
export function ordinary(): IndexSnapshotPayload {
  return { content: content() };
}

/** The Snapshot that activated this epoch, at the head it took (spec: MR-2). */
export function activating(): IndexSnapshotPayload {
  return {
    content: content(),
    activation: {
      baseHeadGeneration: Generation.of(GENERATION.value - 1n),
      activationSlot: 'minted-head-7',
    },
  };
}

/** One Entry of the Library, held by the Container with the given seed. */
export function located(
  seed: number,
  path: string,
  offset: bigint,
  size: bigint,
): EntryLocation {
  return { containerId: containerId(seed), entry: entry(path, offset, size) };
}

/**
 * The content as the encoder puts it on the wire (spec: FM-16).
 *
 * The sample's paths are ASCII, where `<` and the UTF-8 byte order EP-3 calls
 * for agree; the case that covers a path they disagree on computes its own
 * expectation from the bytes rather than from this.
 */
export function canonical(source: SnapshotContent): SnapshotContent {
  return {
    checkpoint: source.checkpoint,
    containers: [...source.containers].sort((left, right) =>
      left.id.toHex() < right.id.toHex() ? -1 : 1,
    ),
    entries: [...source.entries].sort((left, right) =>
      left.entry.path < right.entry.path ? -1 : 1,
    ),
  };
}

/**
 * The fields of a payload body, keyed by name, for a case that has to change
 * one of them: the reader's translation of the body's field numbers for the map
 * kind `kind` (spec: FM-9).
 */
export function bodyMap(payload: ControlPayload, kind: MapFields): Map<unknown, unknown> {
  const map = fromWire(decodeCborExact(payload.body, 'malformed_control_payload'), kind);
  if (!(map instanceof Map)) {
    throw new Error('a payload body is a CBOR map');
  }
  return map;
}

/**
 * A payload carrying the fields a case built by hand: each name `kind`
 * declares under its field number, and any other map key as the case wrote it.
 */
export function withBodyMap(
  masterKeyEpoch: MasterKeyEpoch,
  map: Map<unknown, unknown>,
  kind: MapFields,
): ControlPayload {
  return withRawBody(masterKeyEpoch, toWireLenient(map, kind));
}

/** A payload carrying exactly the value handed in, keys and all. */
export function withRawBody(masterKeyEpoch: MasterKeyEpoch, body: unknown): ControlPayload {
  return { masterKeyEpoch, body: encodeCbor(body, 'control_payload_encode_failed') };
}

/**
 * The same payload with every field, at every level, carried under its name as
 * a text key — the spelling a map keyed by field names would have, which is no
 * field's spelling (spec: FM-9).
 */
export function spelledByName(payload: ControlPayload, kind: MapFields): ControlPayload {
  return withRawBody(
    payload.masterKeyEpoch,
    fromWire(decodeCborExact(payload.body, 'malformed_control_payload'), kind),
  );
}

/** The elements one array field of a map carries. */
export function arrayField(map: Map<unknown, unknown>, key: string): unknown[] {
  const value = map.get(key);
  if (!Array.isArray(value)) {
    throw new Error(`${key} is an array`);
  }
  return value;
}

/** One element of an array field, as the map it is. */
export function mapAt(items: unknown[], index: number): Map<unknown, unknown> {
  const item = items[index];
  if (!(item instanceof Map)) {
    throw new Error(`element ${index} is a map`);
  }
  return item;
}
