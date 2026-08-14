# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that owns the
[Key Envelopes](../key-envelope/) needed to open all current
[Containers](../container/). It exists for two reasons: envelopes must live
outside the Containers so that rotating the
[Master Key](../master-key/) rewrites only megabytes of compact control
objects instead of every Container — and an envelope, unlike the Index,
cannot be rebuilt from anything else, so one object must own the
authoritative envelope set, replicated and explicitly selected, or a single
lost object could silently make a Container unreadable forever.

One logical Keyring **generation** is stored as a **replica set** of several
independently encrypted objects. Every generation belongs to one Master Key
epoch and numbers the envelope-set checkpoints within it.

## Mental Model

A replica is one independently encrypted object carrying a generation's
complete envelope set. The element-level property:

- A replica is **valid** when it decrypts and authenticates and its metadata
  and payload are internally consistent
  ([spec: KL-1](../../spec/keyring-lifecycle/)).

A generation's replica set moves through one lifecycle
([spec: KL-2 to KL-5](../../spec/keyring-lifecycle/)):

| State | Meaning | Entered when |
| --- | --- | --- |
| partial candidate | some replicas written; no commit selects the set | a writer starts preparing the generation |
| complete set | every declared replica is valid and they all agree | preparation and read-back verification finish |
| committed | a Journal commit or epoch activation selected the set's exact commitment tuple | the selecting commit succeeds |
| degraded | committed, but object loss left fewer valid replicas than its tuple requires | replicas are lost or corrupted |
| repaired | restored to the full committed replica count | missing replicas are rewritten before the next write |

Commitment is a selection made by the [Journal](../journal/); validity and
completeness describe only the objects themselves. That split is why a
complete candidate still needs a commit to matter, and why an interrupted
upload leaves harmless partial candidates rather than damaged Keyrings.

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

- A Key Envelope is **irreplaceable**: it cannot be rebuilt from a Container,
  the Journal, or the Master Key, so every generation is stored as several
  replicas — initially three
  ([spec: KL-6, KL-8](../../spec/keyring-lifecycle/)).
  - Replication counts within a generation: a newer generation's envelopes
    are protected only by that generation's own replicas
    ([spec: KL-9](../../spec/keyring-lifecycle/)).
- The committed Keyring holds exactly one envelope for every current
  Container and none for a non-current one, so the current Library opens with
  nothing beyond the Keyring and the Master Key
  ([spec: KL-7](../../spec/keyring-lifecycle/)).
- Each Journal commit selects the exact generation whose envelopes match the
  post-commit Container set; because selection is part of the commit itself,
  membership and keys can never disagree
  ([spec: CP-8 to CP-11](../../spec/commit-protocol/),
  [KL-3, KL-4](../../spec/keyring-lifecycle/)).
- Restore can proceed from any one committed valid replica; a degraded set is
  repaired to the full count before the next write, `prune`, or rotation,
  because writing on thin redundancy would gamble the only copies of
  irreplaceable keys ([spec: KL-11](../../spec/keyring-lifecycle/)).
- A complete replica set that no commit ever selected is a candidate orphan,
  disposed of under the Journal's cleanup rules
  ([spec: KL-12](../../spec/keyring-lifecycle/),
  [OC-2 to OC-5](../../spec/orphan-cleanup/)).
- Journal records become prunable only once an Index Snapshot preserves the
  selected commitment and its replica set is complete, because after `prune`
  the Keyring replicas are the only carriers of those envelopes
  ([spec: CK-5](../../spec/checkpoint-and-prune/),
  [CP-11](../../spec/commit-protocol/)).
- Losing every object that carries a current Container's envelope loses that
  Container, even with the Master Key and the ciphertext — the accepted
  price of cheap rotation.
  - The replica count protects against object-level loss within one Storage
    account, not loss of the Storage account itself.
- On rotation, every old-epoch Keyring, Journal record, and Index Snapshot is
  permanently deleted rather than trashed, because old-epoch control objects
  are exactly what a leaked old Recovery Code could open
  ([spec: MR-3](../../spec/master-key-rotation/)).
- A Keyring is encrypted directly with a purpose-specific key derived from
  the Master Key, so recovery can open the Keyring without already having the
  Keyring.

## Related Concepts

- [Key Envelope](../key-envelope/) — what the Keyring collects
- [Journal](../journal/) — atomically selects an exact Keyring commitment
- [Master Key](../master-key/) — what rotation replaces
- [Storage](../storage/) — where the Keyring lives
- [Storage Object](../storage-object/) — the broader object category a
  Keyring belongs to
