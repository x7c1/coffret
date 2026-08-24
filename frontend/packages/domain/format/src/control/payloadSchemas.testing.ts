/**
 * Test support: the values the payload schema cases are built out of.
 *
 * They are deliberately dull — a Container ID is one byte repeated and a hash is
 * another — because none of what the cases assert turns on what is in them. What
 * matters is that they are distinguishable in a failure message and that they
 * are handed over *out* of the canonical order, so an encoder that left the
 * order alone would be caught (FM-15, FM-16, FM-17).
 *
 * Excluded from the package build — nothing here ships.
 */

import { decodeCborExact, encodeCbor } from '../internal/cbor.js';
import { ContainerId } from '../model/containerId.js';
import { Generation } from '../model/generation.js';
import { KEY_ENVELOPE_LENGTH, KeyEnvelope } from '../model/keyEnvelope.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import type { ContainerSummary } from '../model/containerSummary.js';
import type { EntryLocation } from '../model/entryLocation.js';
import type { EntryMetadata } from '../model/entry.js';
import type { IndexCheckpoint, KeyringCommitment } from '../model/indexCheckpoint.js';
import type { ContainerAddition, JournalRecord } from '../model/journalRecord.js';
import type { KeyringMapping } from '../model/keyringMapping.js';
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
 * not, so a set of two covers both spellings of the optional field (CP-11).
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

/** One entry-table element, laid at `offset` and `size` bytes long (FM-9). */
export function entry(path: string, offset: bigint, size: bigint): EntryMetadata {
  return {
    path,
    offset,
    size,
    mtimeSeconds: 1_700_000_000n,
    hash: contentHash(0x5b),
  };
}

/** The Keyring commitment a commit at `generation` selects (KL-3). */
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

/** One Container a record adds, with an entry table laid end to end (FM-4). */
export function addition(seed: number, kind: ContainerKind): ContainerAddition {
  const label = seed.toString(16).padStart(2, '0');
  const entries = [entry(`albums/${label}/cover.jpg`, 0n, 120n)];
  if (kind === 'pack') {
    entries.push({
      ...entry(`albums/${label}/.thumbs/cover.jpg`, 120n, 40n),
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
 * than what a caller happened to hold (FM-15).
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
 * A name-keyed Storage mints no identifier, so both slots are absent here (CP-2,
 * CP-15) — and generation 0 has no predecessor to state (FM-13).
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
 * A Keyring mapping holding both of the things a Keyring can hold.
 *
 * Two Containers open through an envelope and one is recorded key-lost (KL-7),
 * and the entries are handed over out of Container ID order on purpose: a case
 * comparing bytes is then comparing what the encoder ordered rather than what a
 * caller happened to hold (FM-17).
 */
export function mapping(): KeyringMapping {
  return {
    entries: [
      { containerId: containerId(0x40), key: { status: 'envelope', envelope: envelope(0x40) } },
      { containerId: containerId(0x99), key: { status: 'key-lost' } },
      { containerId: containerId(0x21), key: { status: 'envelope', envelope: envelope(0x21) } },
    ],
  };
}

/**
 * The mapping whose digest both implementations pin.
 *
 * Deliberately smaller and duller than {@link mapping}: it exists so that the
 * two implementations state one expected digest each, in a shape that is easy to
 * spell identically in both languages. The Rust suite builds the same two
 * entries — `11…` with an envelope of `22` bytes, `33…` key-lost — and asserts
 * the same hex.
 */
export function pinnedMapping(): KeyringMapping {
  return {
    entries: [
      { containerId: containerId(0x11), key: { status: 'envelope', envelope: envelope(0x22) } },
      { containerId: containerId(0x33), key: { status: 'key-lost' } },
    ],
  };
}

/**
 * A Library of three Containers whose Entries interleave across them.
 *
 * Interleaving is the point: `entries` is in Entry Path order across the whole
 * Library (EP-3), not grouped by Container, so a case comparing the encoded
 * order to the order the content was handed over in has something to catch.
 */
export function content(): SnapshotContent {
  return {
    checkpoint: checkpoint(GENERATION.value),
    containers: [summary(0x40, 'pack'), summary(0x21, 'one-file'), summary(0x33, 'pack')],
    entries: [
      located(0x33, 'photos/2019/b.jpg', 0n, 90n),
      located(0x40, 'albums/spring/a.jpg', 0n, 100n),
      located(0x21, 'books/atlas/page-001.png', 0n, 200n),
      located(0x40, 'photos/2019/a.jpg', 100n, 80n),
    ],
  };
}

/** The ordinary checkpoint of that head (CK-10). */
export function ordinary(): IndexSnapshotPayload {
  return { content: content() };
}

/** The Snapshot that activated this epoch, at the head it took (MR-2). */
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
 * The content as the encoder puts it on the wire (FM-16).
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

/** The fields of a payload body, for a case that has to change one of them. */
export function bodyMap(payload: ControlPayload): Map<unknown, unknown> {
  const map = decodeCborExact(payload.body, 'malformed_control_payload');
  if (!(map instanceof Map)) {
    throw new Error('a payload body is a CBOR map');
  }
  return map;
}

/** A payload carrying the fields a case built by hand. */
export function withBodyMap(
  masterKeyEpoch: MasterKeyEpoch,
  map: Map<unknown, unknown>,
): ControlPayload {
  return { masterKeyEpoch, body: encodeCbor(map, 'control_payload_encode_failed') };
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
