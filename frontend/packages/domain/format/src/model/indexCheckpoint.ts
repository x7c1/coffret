import { fail } from '../errors.js';
import type { Generation } from './generation.js';
import type { MasterKeyEpoch } from './masterKeyEpoch.js';

/**
 * The exact Keyring replica set a commit selected (CP-10, KL-3).
 *
 * A replica set becomes committed only when a Journal commit or an epoch
 * activation names its whole tuple, and a candidate carrying any other
 * commitment is not selected even at the same generation. The tuple therefore
 * travels as one value; the Master Key epoch belongs to it too and is held once
 * by the checkpoint this is part of, because a checkpoint belongs to exactly one
 * epoch (CK-3).
 */
export interface KeyringCommitment {
  /** Which generation of the Keyring the committed set belongs to. */
  generation: Generation;
  /** How many replicas that generation declares (KL-2). */
  replicaCount: number;
  /** The digest binding the canonical complete mapping (KL-1, CP-10). */
  setDigest: string;
}

/**
 * Checks a commitment crossing this package's boundary, in either direction.
 *
 * The digest is a non-empty lowercase hex token, the same spelling a replica's
 * object name carries it in (FM-12) — two spellings of one digest would name one
 * replica set twice, while a commit selects a set by its exact tuple. A count of
 * zero declares no replica and so can never be complete (KL-2).
 *
 * An encoder calls it too, unlike the readers of the fields around it: the
 * commitment is a plain interface here rather than a value that cannot be built
 * wrong, so a caller can hand an encoder one that every decoder — this package's
 * and the Rust implementation's — would refuse. Catching it on the way out
 * raises the same code the way in raises, at the device that could still fix it.
 */
export function requireKeyringCommitment(commitment: KeyringCommitment): KeyringCommitment {
  if (commitment.replicaCount <= 0 || !Number.isSafeInteger(commitment.replicaCount)) {
    fail('invalid_replica_count', 'a Keyring replica set declares at least one replica');
  }
  if (!/^[0-9a-f]+$/.test(commitment.setDigest)) {
    fail(
      'invalid_set_digest',
      `${JSON.stringify(commitment.setDigest)} is not a lowercase hex Keyring digest`,
    );
  }
  return commitment;
}

/**
 * The committed Library state an Index stands at (CK-1, CK-2, CK-3).
 *
 * The two generations are not the same number and are both needed: recovery
 * starts from the head generation and replays the Journal successors after it,
 * while the last applied Journal generation is what says which records have
 * become eligible for `prune`. They coincide after an ordinary commit and
 * diverge after an epoch activation, whose Snapshot occupies a head position
 * without being a Journal record (CP-6, FM-12).
 */
export interface IndexCheckpoint {
  /** Which Master Key encrypted the control state this stands on (CK-3, FM-13). */
  masterKeyEpoch: MasterKeyEpoch;
  /** The control-head generation this checkpoint represents (CK-1). */
  headGeneration: Generation;
  /** The last Journal generation applied to reach it (CK-1, CK-4). */
  journalGeneration: Generation;
  /**
   * The slot this head's successor is committed into, as Storage's own opaque
   * token (CK-2, CP-2).
   *
   * Absent where the provider keys objects by name and so mints nothing: there
   * the slot is the successor's name, re-derived at spend time rather than
   * persisted, so the two spellings cannot drift apart (CP-15).
   */
  nextCommitSlot?: string;
  /** The exact Keyring replica set the commit behind this head selected. */
  keyring: KeyringCommitment;
}
