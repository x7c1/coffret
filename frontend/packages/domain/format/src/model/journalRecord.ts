import type { ContainerId } from './containerId.js';
import type { ContainerSummary } from './containerSummary.js';
import type { EntryMetadata } from './entry.js';
import type { Generation } from './generation.js';
import type { KeyringCommitment } from './indexCheckpoint.js';
import type { MasterKeyEpoch } from './masterKeyEpoch.js';

/**
 * One Container a Journal record adds, with everything it holds (spec: CP-11).
 *
 * A record carries each new Container's ciphertext hash, its kind, and its entry
 * table — the values the meta section records, under the catalog's own field
 * names (spec: FM-15) — which is exactly what lets a device replaying the record
 * rebuild its Index without opening a single Container (spec: CK-9, RV-5).
 *
 * No Key Envelope ever rides here: which Containers are current is the Journal's
 * business, and the committed Keyring is the only Storage home of the keys that
 * open them.
 */
export interface ContainerAddition {
  /** What the record records about the Container itself. */
  container: ContainerSummary;
  /**
   * The Container's entry table, in plaintext stream order (spec: FM-9, FM-15).
   */
  entries: EntryMetadata[];
}

/**
 * One committed Journal record, as the Index replays it (spec: FM-15).
 *
 * The record is the commit point of a batch: before it exists the batch has
 * changed nothing, and once it exists its additions and removals are part of the
 * current Container set, never partially (spec: CP-1).
 */
export interface JournalRecord {
  /**
   * The head-chain generation this record becomes on committing
   * (spec: CP-2, FM-13).
   */
  generation: Generation;
  /**
   * The generation of the control head this record succeeds, absent at
   * generation 0 where the Library has no earlier head (spec: FM-13, FM-15).
   */
  prev?: Generation;
  /** The Master Key epoch this record belongs to (spec: CP-13, FM-13). */
  masterKeyEpoch: MasterKeyEpoch;
  /** The exact Keyring replica set this commit selects (spec: CP-10, KL-3). */
  keyring: KeyringCommitment;
  /**
   * The slot this record's own successor is committed into, as Storage's opaque
   * token, and absent where the provider mints none (spec: CP-2, CP-15).
   */
  nextCommitSlot?: string;
  /**
   * The one slot this head's ordinary Index Snapshot may be created into,
   * reserved before the commit in the same form as the commit slot
   * (spec: CK-10).
   */
  snapshotSlot?: string;
  /** The Containers the batch added, with their entry tables (spec: CP-11). */
  additions: ContainerAddition[];
  /**
   * The Containers the batch removed.
   *
   * A removed Container ID is never added again, so removal from the current set
   * is monotonic and replaying a record twice removes the same thing twice
   * (spec: CP-14).
   */
  removals: ContainerId[];
}
