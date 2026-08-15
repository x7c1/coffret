# Keyring Lifecycle

Rule prefix: `KL`. When a Keyring replica is valid, when a replica set is
complete, committed, or degraded, what each state permits, and how a
degraded set is repaired.

Concept background: [Keyring](../../concepts/keyring/),
[Key Envelope](../../concepts/key-envelope/).

## Rules

- **KL-1.** A replica is **valid** when it decrypts and authenticates
  successfully, its epoch, generation, replica index and count are internally
  consistent, and its `set_digest` matches the canonical complete mapping
  from Container IDs to Key Envelopes and key-lost markers (KL-7) in its
  payload. *(Form: test)*
- **KL-2.** A replica set is **complete** when its valid replicas agree on
  one epoch, generation, replica count, and `set_digest`, and every replica
  index declared by that count is present exactly once. A candidate set can
  reach completeness before any commit selects it. *(Form: test)*
- **KL-3.** A replica set becomes **committed** only when a successful
  Journal commit or Master Key epoch activation selects its exact commitment
  tuple — `master_key_epoch`, generation, replica count, and `set_digest`
  (CP-10). A valid replica matching that tuple is a committed valid replica.
  *(Form: test)*
- **KL-4.** An ordinary Index Snapshot records a tuple that was already
  committed; after covered Journal records are pruned, the Snapshot preserves
  the evidence of that earlier selection. Placing a reference to a candidate
  set in a Snapshot leaves the candidate uncommitted. *(Form: test)*
- **KL-5.** A committed replica set with fewer valid replicas than the count
  its commitment selected is **degraded**. An incomplete uncommitted set is a
  **partial candidate**, a distinct state from degraded. *(Form: test)*
- **KL-6.** One committed valid replica contains the complete logical Keyring
  payload; the replica count provides redundancy against individual object
  loss and carries no quorum semantics. *(Form: test)*
- **KL-7.** At every successful commit or `prune` boundary, the committed
  Keyring maps every current Container and no non-current one. Each is
  mapped either to its Key Envelope or to an explicit **key-lost marker**
  recording that no copy of the key survives. *(Form: test)*
- **KL-8.** A generation's commitment selects the replica count required for
  that generation; a newly prepared generation uses the current replica
  policy, whose initial value is three. *(Form: test)*
- **KL-9.** Replication is effective only within one generation: envelopes
  introduced by a newer generation are protected only by that generation's
  replicas, never by retained older generations. *(Form: test)*
- **KL-10.** Every Keyring generation belongs to one `master_key_epoch`; the
  generation numbers the successive envelope sets within that epoch and is
  distinct from the epoch itself. *(Form: test)*
- **KL-11.** Restore may use any one committed valid replica. If fewer than
  the committed count remain, restore proceeds with the degraded set, but the
  set must be repaired to complete before another write, `prune`, or Master
  Key rotation. *(Form: test)*
- **KL-12.** A valid replica set with no reachable committed Journal record
  or Index Snapshot is treated as a suspected orphan; its
  disposal follows the orphan-cleanup rules (OC-2 to OC-5). *(Form: test)*
- **KL-13.** Repair is automatic: whichever device finds the committed
  replica set degraded (KL-5) rewrites the missing replicas from any
  committed valid replica, restoring the full committed count. Repair only
  re-materializes the committed generation — it never invents state and
  never deletes anything. *(Form: test)*
- **KL-14.** Replica objects are identified by epoch, generation, and
  replica index, and repair writes use create-if-absent semantics per slot,
  so concurrent repairs by multiple devices are benign. *(Form: test)*
- **KL-15.** Replica loss and the repair performed are surfaced to the user
  as a health event; neither happens silently. *(Form: test)*
- **KL-16.** If repair cannot complete — write failures, quota, permissions
  — the completeness gate holds unchanged: writes, `prune`, and Master Key
  rotation remain refused (KL-11), while reads and restore remain allowed
  from the surviving committed valid replicas. The failure is reported and
  retried; the gate is never partially relaxed. *(Form: test)*
- **KL-17.** A current Container mapped to a key-lost marker by the committed
  Keyring remains current: nothing authorizes deleting its ciphertext, and it
  leaves the current set only through a genuine committed removal — the user
  deletes it, or `update` replaces it, for which a key-lost Container is
  always eligible while its local file survives (PK-11, PK-12), healing the
  loss. *(Form: test)*
