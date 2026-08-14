# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that owns the
[Key Envelopes](../key-envelope/) needed to open all current
[Containers](../container/). One logical Keyring generation is stored as a
replica set of several independently encrypted objects. Rewriting the Keyring
together with the other compact control objects is what makes rotating the
[Master Key](../master-key/) a megabytes-scale operation instead of rewriting
all Containers.

## Examples

- A Library of ten thousand Containers has a Keyring payload of roughly 1 MB;
  the initial three-replica policy stores roughly 3 MB
- A leaked [Recovery Code](../recovery-code/) — a photographed paper — is
  neutralized, if the attacker has not reached Storage, by activating a new
  epoch and permanently deleting every old-epoch Keyring, Journal record, and
  Index Snapshot that coffret can reach. Copies retained by an attacker or the
  Storage provider cannot be invalidated by rotation

## Collocations

- rewrite (the Keyring when rotating the Master Key)
- prepare (a Keyring for the post-commit Container set)
- replicate (a Keyring generation before committing its Journal generation)
- fetch (the Keyring first, when recovering)

## Domain Rules

- A Key Envelope is **irreplaceable**: unlike the Index, it cannot be rebuilt
  from a Container, the Journal, or the Master Key. At every successful commit
  or `prune` boundary, the committed Keyring must contain exactly one envelope
  for every current Container and no envelope for a non-current Container.
  Its commitment selects the replica count required for that generation; a
  newly prepared generation uses the current replica policy, whose initial
  value is three.
- Generations and replicas are different. A generation is a logical snapshot
  of the envelope set; replicas are independently encrypted Storage objects
  containing that same snapshot. Retaining older generations does not count
  as replication for envelopes introduced by a newer generation.
- A replica is **valid** when it decrypts and authenticates successfully, its
  epoch, generation, replica index and count are internally consistent, and
  its `set_digest` matches the canonical complete mapping from Container IDs
  to Key Envelopes in its payload.
- A replica set is **complete** when its valid replicas agree on one epoch,
  generation, replica count, and `set_digest`, and every replica index declared
  by that count is present exactly once.
  Completeness does not depend on whether the generation has been committed: a
  candidate set can be complete before a Journal commit or Master Key epoch
  activation selects it.
- A Keyring replica set becomes **committed** only when a successful Journal
  commit or Master Key epoch activation selects its exact commitment tuple:
  `master_key_epoch`, generation, replica count, and `set_digest`. A valid
  replica matching that tuple is a committed valid replica. An ordinary Index
  Snapshot only records a tuple that was already committed; after covered
  Journal records are pruned, the Snapshot preserves the evidence of that
  earlier selection. Merely placing a reference in a Snapshot does not commit
  a candidate set.
- If a committed replica set has fewer valid replicas than the count selected
  by its commitment, its replica set is **degraded**. An incomplete
  uncommitted set is instead a partial candidate and is not called degraded.
  Cryptographic validity or
  completeness alone does not make a replica committed.
- The replica count provides redundancy against individual object loss, not a
  quorum: one committed valid replica contains the complete logical Keyring
  payload.
- Every Keyring generation belongs to one `master_key_epoch`. Its generation
  tracks envelope-set checkpoints within that epoch and is not itself a
  Master Key epoch.
- Restore may use any one committed valid replica. If fewer than the committed
  replica count remain, restore proceeds with a degraded set, but coffret
  repairs it to a complete set before allowing another write, `prune`, or
  Master Key rotation. A valid replica set with no reachable committed Journal
  record or Index Snapshot is ignored as a candidate uncommitted orphan. Its
  disposal follows the [Journal](../journal/)'s orphan-cleanup rules.
- A Journal record may be deleted by `prune` only after its corresponding
  Index Snapshot preserves the selected Keyring commitment and that exact
  Keyring replica set is complete. Journal records never serve as envelope
  copies, before or after `prune`.
- Losing every object that carries a current Container's envelope loses that
  Container, even with the Master Key and Container ciphertext — the accepted
  price of cheap rotation. The replica count protects against object-level
  loss within one Storage account, not loss of the Storage account itself.
- On rotation, every old-epoch Keyring, Journal record, and Index Snapshot is
  permanently deleted, not trashed. Rotation is not complete while any such
  control object remains reachable. This invalidates a leaked Recovery Code
  only to the extent that neither an attacker nor the Storage provider retained
  a copy before deletion.
- A Keyring has no Container Key or Key Envelope of its own. It is encrypted
  and authenticated directly with a purpose-specific key derived from the
  Master Key, so recovery can open the Keyring without already having the
  Keyring.

## Related Concepts

- [Key Envelope](../key-envelope/) — what the Keyring collects
- [Journal](../journal/) — atomically selects an exact Keyring commitment
- [Master Key](../master-key/) — what rotation replaces
- [Storage](../storage/) — where the Keyring lives
- [Storage Object](../storage-object/) — the broader object category a
  Keyring belongs to
