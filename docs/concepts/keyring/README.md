# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that
checkpoints the [Key Envelopes](../key-envelope/) needed to open all current
[Containers](../container/). One logical Keyring generation is stored as a
replica set of several independently encrypted objects. Rewriting the Keyring
together with the other compact control objects is what makes rotating the
[Master Key](../master-key/) a megabytes-scale operation instead of rewriting
all Containers.

## Examples

- A Library of ten thousand Containers has a Keyring payload of roughly 1 MB;
  the initial three-replica policy stores roughly 3 MB
- A leaked [Recovery Code](../recovery-code/) — a photographed paper — is
  neutralized by re-wrapping every envelope into a new Keyring and
  permanently deleting the old one, before the attacker ever reaches Storage

## Collocations

- rewrite (the Keyring when rotating the Master Key)
- checkpoint (the Journal's envelopes into the Keyring)
- replicate (a Keyring generation before committing its Journal generation)
- fetch (the Keyring first, when recovering)

## Domain Rules

- A Key Envelope is **irreplaceable**: unlike the Index, it cannot be rebuilt
  from a Container or the Master Key. At every successful commit or `prune`
  boundary, every envelope needed by a current Container must therefore exist
  in at least the configured number of valid replicas; the replica count is a
  policy parameter whose initial value is three.
- Generations and replicas are different. A generation is a logical snapshot
  of the envelope set; replicas are independently encrypted Storage objects
  containing that same snapshot. Retaining older generations does not count
  as replication for envelopes introduced by a newer generation.
- A replica is **valid** when it decrypts and authenticates successfully and
  its epoch, generation, replica index and count, and `set_digest` are
  internally consistent.
- A replica set is **complete** when its valid replicas agree on one epoch,
  generation, replica count, and `set_digest`, and every replica index declared
  by that count is present exactly once.
  Completeness does not depend on whether the generation has been committed: a
  candidate set can be complete before a Journal record or Index Snapshot
  refers to it.
- A valid replica is **committed** when an authenticated Journal record or Index
  Snapshot in the current control history references its generation and
  `set_digest`. If a committed generation has fewer valid replicas than its
  declared count, its replica set is **degraded**. An incomplete uncommitted set
  is instead a partial candidate and is not called degraded. Cryptographic
  validity or completeness alone does not make a replica committed.
- The replica count provides redundancy against individual object loss, not a
  quorum: one committed valid replica contains the complete logical Keyring
  payload.
- Every Keyring generation belongs to one `master_key_epoch`. Its generation
  tracks envelope-set checkpoints within that epoch and is not itself a
  Master Key epoch.
- Restore may use any one committed valid replica. If fewer than the configured
  number remain, restore proceeds with a degraded set, but coffret repairs it
  to a complete set before allowing another write, `prune`, or Master Key
  rotation. A valid replica set with no reachable committed Journal record or
  Index Snapshot is ignored as a candidate uncommitted orphan. Its disposal
  follows the [Journal](../journal/)'s orphan-cleanup rules.
- A Journal record may be deleted by `prune` only after a complete Keyring
  replica set covers every envelope that will remain reachable after the
  corresponding Index Snapshot. After `prune`, the replica set alone still
  satisfies the envelope-copy invariant.
- Losing every object that carries a current Container's envelope loses that
  Container, even with the Master Key and Container ciphertext — the accepted
  price of cheap rotation. The replica count protects against object-level
  loss within one Storage account, not loss of the Storage account itself.
- On rotation, old-epoch Keyrings are permanently deleted, not trashed: they
  are exactly what a leaked Recovery Code could open.
- A Keyring has no Container Key or Key Envelope of its own. It is encrypted
  and authenticated directly with a purpose-specific key derived from the
  Master Key, so recovery can open the Keyring without already having the
  Keyring.

## Related Concepts

- [Key Envelope](../key-envelope/) — what the Keyring collects
- [Journal](../journal/) — carries envelopes between checkpoints
- [Master Key](../master-key/) — what rotation replaces
- [Storage](../storage/) — where the Keyring lives
- [Storage Object](../storage-object/) — the broader object category a
  Keyring belongs to
