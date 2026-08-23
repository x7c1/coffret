/**
 * The names control objects are stored under (FM-12).
 *
 * ```text
 * head-<generation>.cfrt                                 a link in the control-head chain
 * idx-<generation>.cfrt                                  an ordinary Index Snapshot
 * key-<generation>-<set_digest>-r<index>-of-<count>.cfrt  a Keyring replica
 * ```
 *
 * Control objects carry recognizable names because recovery discovers them by
 * name before any index exists.
 *
 * A name says what an object is **for**, not what it **is**: the head chain, an
 * ordinary checkpoint, a Keyring replica. Which kind an object is rides in its
 * authenticated header (FM-11), because one head position admits two kinds — the
 * ordinary Journal record and the Index Snapshot that activates a new Master Key
 * epoch both compete for the same successor slot, so naming them differently
 * would leave two keys where the commit protocol needs one (CP-2, CP-3).
 * {@link nameAdmitsKind} is the whole of that relation, and parsing a name
 * therefore yields no kind at all.
 *
 * A link in the head chain and an Index Snapshot are written once each, so their
 * names carry no replica position and they report replica index 0, count 1.
 *
 * Numbers are spelled in decimal without leading zeros, so one object has
 * exactly one name: a reader that accepted `head-007.cfrt` as generation 7 would
 * let two names claim the same object.
 */

import { U16_MAX, U64_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';
import { Generation } from '../model/generation.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import { STORAGE_EXTENSION } from '../model/containerId.js';
import type { ControlObjectKind } from '../model/kinds.js';

/** The name prefix of a link in the control-head chain. */
const HEAD_PREFIX = 'head-';
/** The name prefix of an ordinary Index Snapshot. */
const INDEX_SNAPSHOT_PREFIX = 'idx-';
/** The name prefix of a Keyring replica. */
const KEYRING_PREFIX = 'key-';

/** What an object's name says it is stored for. */
export type ControlObjectRole = 'head' | 'index-snapshot' | 'keyring-replica';

/** The name a control object is stored under. */
export interface ControlObjectName {
  /** What this name says the object is stored for. */
  role: ControlObjectRole;
  /** The generation the name encodes. */
  generation: Generation;
  /** The replica position the name encodes. */
  replica: ReplicaPosition;
  /**
   * The digest of the mapping the replica set carries, on a Keyring replica.
   *
   * Its contents are the Keyring's business; a name only needs it to be a
   * lowercase hex token, so that it cannot swallow the separators the rest of
   * the name is parsed on.
   */
  setDigest?: string;
}

/** The name of one generation of the control-head chain. */
export function headName(generation: Generation): ControlObjectName {
  return { role: 'head', generation, replica: ReplicaPosition.SINGLE };
}

/**
 * The name the successor of the head at `generation` is created under.
 *
 * Both successor kinds derive the same name from the same head, which is what
 * makes the conditional create that settles a commit a race between them rather
 * than two uncontested writes (CP-2, CP-3, FM-13).
 */
export function successorName(generation: Generation): ControlObjectName {
  return headName(generation.next());
}

/** The name of the ordinary Index Snapshot checkpointing one head. */
export function indexSnapshotName(generation: Generation): ControlObjectName {
  return { role: 'index-snapshot', generation, replica: ReplicaPosition.SINGLE };
}

/** The name of one replica of one generation of the Keyring. */
export function keyringReplicaName(
  generation: Generation,
  setDigest: string,
  replica: ReplicaPosition,
): ControlObjectName {
  return {
    role: 'keyring-replica',
    generation,
    replica,
    setDigest: requireSetDigest(setDigest),
  };
}

/** Which kinds each name form may carry (FM-12). */
const ADMITTED: Readonly<Record<ControlObjectRole, readonly ControlObjectKind[]>> = {
  head: ['journal', 'activation-snapshot'],
  'index-snapshot': ['index-snapshot'],
  'keyring-replica': ['keyring'],
};

/**
 * Whether an object of `kind` may be stored under `name` (FM-12).
 *
 * Every pairing outside this table is refused before decryption.
 */
export function nameAdmitsKind(name: ControlObjectName, kind: ControlObjectKind): boolean {
  return ADMITTED[name.role].includes(kind);
}

/** Spells a name as the string the object is stored under. */
export function formatControlObjectName(name: ControlObjectName): string {
  switch (name.role) {
    case 'head':
      return `${HEAD_PREFIX}${name.generation}${STORAGE_EXTENSION}`;
    case 'index-snapshot':
      return `${INDEX_SNAPSHOT_PREFIX}${name.generation}${STORAGE_EXTENSION}`;
    case 'keyring-replica': {
      const digest = requireSetDigest(name.setDigest);
      const position = `r${name.replica.index}-of-${name.replica.count}`;
      return `${KEYRING_PREFIX}${name.generation}-${digest}-${position}${STORAGE_EXTENSION}`;
    }
  }
}

/** Reads a name back into the values it encodes. */
export function parseControlObjectName(name: string): ControlObjectName {
  const malformed: () => never = () =>
    fail('malformed_object_name', `${JSON.stringify(name)} is not a control-object name`);

  if (!name.endsWith(STORAGE_EXTENSION)) {
    malformed();
  }
  const body = name.slice(0, -STORAGE_EXTENSION.length);

  if (body.startsWith(HEAD_PREFIX)) {
    return headName(parseGeneration(body.slice(HEAD_PREFIX.length), malformed));
  }
  if (body.startsWith(INDEX_SNAPSHOT_PREFIX)) {
    return indexSnapshotName(parseGeneration(body.slice(INDEX_SNAPSHOT_PREFIX.length), malformed));
  }
  if (!body.startsWith(KEYRING_PREFIX)) {
    malformed();
  }

  // The digest is hex, so it holds none of the `-` this splits on and the five
  // fields always land in the same places.
  const fields = body.slice(KEYRING_PREFIX.length).split('-');
  if (fields.length !== 5) {
    malformed();
  }
  const [generation, setDigest, index, of, count] = fields;
  if (of !== 'of' || !index.startsWith('r')) {
    malformed();
  }
  if (setDigest === '' || !isLowercaseHex(setDigest)) {
    malformed();
  }
  return keyringReplicaName(
    parseGeneration(generation, malformed),
    setDigest,
    ReplicaPosition.of(parseCount(index.slice(1), malformed), parseCount(count, malformed)),
  );
}

/** Whether two names name the same object. */
export function controlObjectNamesEqual(left: ControlObjectName, right: ControlObjectName): boolean {
  return (
    left.role === right.role &&
    left.generation.equals(right.generation) &&
    left.replica.equals(right.replica) &&
    left.setDigest === right.setDigest
  );
}

function requireSetDigest(setDigest: string | undefined): string {
  // Lowercase only, as every hex spelling in coffret is: two spellings of one
  // digest would be two names for one object.
  if (setDigest === undefined || setDigest === '' || !isLowercaseHex(setDigest)) {
    fail(
      'malformed_object_name',
      `a Keyring replica name carries a lowercase hex digest, found ${JSON.stringify(setDigest)}`,
    );
  }
  return setDigest;
}

function isLowercaseHex(text: string): boolean {
  return /^[0-9a-f]+$/.test(text);
}

/** Reads a decimal number that carries no leading zeros and no sign. */
function parseDigits(digits: string, malformed: () => never): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(digits)) {
    malformed();
  }
  return BigInt(digits);
}

function parseGeneration(digits: string, malformed: () => never): Generation {
  const value = parseDigits(digits, malformed);
  if (value > U64_MAX) {
    malformed();
  }
  return Generation.of(value);
}

function parseCount(digits: string, malformed: () => never): number {
  const value = parseDigits(digits, malformed);
  if (value > BigInt(U16_MAX)) {
    malformed();
  }
  return Number(value);
}
