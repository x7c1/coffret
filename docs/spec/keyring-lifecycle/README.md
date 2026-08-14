# Keyring Lifecycle

Rule prefix: `KL`. When a Keyring replica is valid, when a replica set is
complete, committed, or degraded, and what each state permits.

Concept background: [Keyring](../../concepts/keyring/),
[Key Envelope](../../concepts/key-envelope/).

## Rules

- **KL-1.** A replica is **valid** when it decrypts and authenticates
  successfully, its epoch, generation, replica index and count are internally
  consistent, and its `set_digest` matches the canonical complete mapping
  from Container IDs to Key Envelopes in its payload. *(→ tests)*
- **KL-2.** A replica set is **complete** when its valid replicas agree on
  one epoch, generation, replica count, and `set_digest`, and every replica
  index declared by that count is present exactly once. A candidate set can
  reach completeness before any commit selects it. *(→ tests)*
- **KL-3.** A replica set becomes **committed** only when a successful
  Journal commit or Master Key epoch activation selects its exact commitment
  tuple — `master_key_epoch`, generation, replica count, and `set_digest`
  (CP-10). A valid replica matching that tuple is a committed valid replica.
  *(→ tests)*
- **KL-4.** An ordinary Index Snapshot records a tuple that was already
  committed; after covered Journal records are pruned, the Snapshot preserves
  the evidence of that earlier selection. Placing a reference to a candidate
  set in a Snapshot leaves the candidate uncommitted. *(→ tests)*
- **KL-5.** A committed replica set with fewer valid replicas than the count
  its commitment selected is **degraded**. An incomplete uncommitted set is a
  **partial candidate**, a distinct state from degraded. *(→ tests)*
- **KL-6.** One committed valid replica contains the complete logical Keyring
  payload; the replica count provides redundancy against individual object
  loss and carries no quorum semantics. *(→ tests)*
- **KL-7.** At every successful commit or `prune` boundary, the committed
  Keyring contains exactly one envelope for every current Container and no
  envelope for a non-current Container. *(→ tests)*
- **KL-8.** A generation's commitment selects the replica count required for
  that generation; a newly prepared generation uses the current replica
  policy, whose initial value is three. *(→ tests)*
- **KL-9.** Replication counts within one generation: envelopes introduced by
  a newer generation are protected only by that generation's replicas, never
  by retained older generations. *(→ tests)*
- **KL-10.** Every Keyring generation belongs to one `master_key_epoch`; the
  generation numbers envelope-set checkpoints within that epoch and is
  distinct from the epoch itself. *(→ tests)*
- **KL-11.** Restore may use any one committed valid replica. If fewer than
  the committed count remain, restore proceeds with the degraded set, but the
  set must be repaired to complete before another write, `prune`, or Master
  Key rotation. *(→ tests)*
- **KL-12.** A valid replica set with no reachable committed Journal record
  or Index Snapshot is treated as a candidate uncommitted orphan; its
  disposal follows the orphan-cleanup rules (OC-2 to OC-5). *(→ tests)*
