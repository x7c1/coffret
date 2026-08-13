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
  in at least the configured number of verified Storage objects; the initial
  replica count is three.
- Generations and replicas are different. A generation is a logical snapshot
  of the envelope set; replicas are independently encrypted Storage objects
  containing that same snapshot. Retaining older generations does not count
  as replication for envelopes introduced by a newer generation.
- Every Keyring generation belongs to one `master_key_epoch`. Its generation
  tracks envelope-set checkpoints within that epoch and is not itself a
  Master Key epoch.
- Before a [Journal](../journal/) record makes new Containers current, coffret
  writes and verifies every replica of a Keyring generation that covers the
  previous current envelopes plus the new additions. A partial replica set
  cannot authorize a Journal commit or `prune`.
- If objects are lost after commit, any remaining authenticated replica can
  be used for recovery. The generation is then degraded: coffret repairs it
  back to the configured replica count before allowing another write or
  `prune`. A replica set with no committed Journal record or Index Snapshot is
  instead an uncommitted orphan and is ignored.
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
