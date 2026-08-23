# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that records
the key status of every current [Container](../container/). It maps each one
either to the [Key Envelope](../key-envelope/) needed to open it or to an
explicit **key-lost marker** when the committed control state has no reachable
envelope for it. Keeping envelopes outside Containers lets
[Master Key](../master-key/) rotation rewrite only compact control objects,
not every Container. Because Container Keys cannot be derived from the other
persisted Storage state, the Keyring is stored as several replicas.

## Mental Model

### Generations and replicas

One logical Keyring **generation** is stored as a **replica set** of several
independently encrypted objects. Every replica carries the generation's
complete mapping: every current Container to its Key Envelope, or to an
explicit key-lost marker when the committed control state has no reachable
envelope for it (spec: KL-7). Reading therefore needs just one valid committed
replica — the count adds redundancy, never a quorum (spec: KL-6). Every
generation belongs to one Master Key epoch; its number runs across epochs
without restarting.

### Keyring commitment

Four values identify one replica set exactly. Together they are its
**Keyring commitment**:

- the Master Key epoch the generation belongs to
- the generation's number, which runs across epochs without restarting
- the replica count the generation was written with
- the **set digest**: a short fixed-size fingerprint computed from the
  mapping, which comes out different if any pair in the mapping changes

The digest is what lets a commitment name the set's exact contents rather
than just its place in the numbering, so two candidates sharing a generation
are never confused.

### State

A replica is one independently encrypted object carrying a generation's
complete mapping. The element-level property:

- A replica is **valid** when it decrypts and authenticates and its metadata
  and payload are internally consistent
  (spec: KL-1).

A replica set has two separate properties: whether it has been selected as
committed, and how many valid replicas are available. Their useful
combinations are
(spec: KL-2, KL-3, KL-4, KL-5, RV-7):

| Name | Selected as committed? | Valid replicas | Meaning |
| --- | --- | --- | --- |
| partial candidate | no | fewer than declared | preparation has not finished |
| complete candidate | no | every declared replica | ready, but not authoritative |
| committed complete | yes | every declared replica | authoritative and fully replicated |
| committed degraded | yes | at least one, but fewer than declared | readable, but redundancy needs repair |
| Keyring loss | yes | none | unreadable and not repairable from Storage alone |

A degraded set is repaired automatically by whichever device detects it —
its missing replicas rewritten until the committed set is complete again —
before the next mutation (spec: KL-11, KL-13). Keyring loss is different:
with no surviving valid replica, ordinary repair has nothing to copy. It
needs a rebuild from authenticated local key material where available
(spec: RV-7, RV-8).

Commitment is a selection: an ordinary [Journal](../journal/) commit makes
it, and so does the activation of a new [Master Key](../master-key/) epoch,
which consumes the same commit slot (spec: KL-3). A valid replica is one good
object; a complete set has all of its declared replicas. Neither fact says
that a commit selected the set. That split is why a complete candidate still
needs a commit to matter, and why an interrupted upload leaves a partial
candidate rather than a degraded Keyring.

## Examples

- A [Library](../library/) of ten thousand Containers has a Keyring payload
  of roughly 1 MB; the initial three-replica policy stores roughly 3 MB
- If an attacker photographs a [Recovery Code](../recovery-code/) — a paper
  backup — but has not yet reached Storage, the leak is neutralized by
  activating a new epoch and permanently deleting every reachable old-epoch
  Keyring, Journal record, and [Index Snapshot](../index-snapshot/). Copies
  already retained by an attacker or the Storage provider cannot be
  invalidated by rotation

## Collocations

- rewrite (the Keyring when rotating the Master Key)
- prepare (a Keyring for the post-commit Container set)
- replicate (a Keyring generation before the Journal commit that selects it)
- fetch (the Keyring first, when recovering)

## Domain Rules

- The committed Keyring maps every current Container and no other, so every
  current Container either opens through its envelope or is visibly recorded
  as key-lost — never silently unreadable (spec: KL-7).
- A replica's name is recognizable, so recovery finds the Keyring before any
  Index exists (spec: FM-12). What the provider still sees despite the
  encrypted, size-padded payload is listed under
  [Storage Object](../storage-object/).
- Losing every Storage object that carries a current Container's envelope
  loses access to that Container's content from the Master Key and ciphertext
  alone. Only authenticated local key material can recreate that Container's
  envelope; a surviving local file can instead recover the content by
  replacing the Container through `update`. This is the accepted price of
  cheap rotation.
  - The replica count protects against object-level loss within one Storage
    account, not loss of the Storage account itself.
  - The affected Containers remain current but locked, recorded with key-lost
    markers until authenticated local key material restores an envelope or a
    surviving local file replaces the Container. Only a committed removal or
    replacement takes them out of the current set (spec: RV-7, RV-8, KL-17).

## Related Concepts

- [Key Envelope](../key-envelope/) — what the Keyring collects
- [Journal](../journal/) — atomically selects an exact Keyring commitment
- [Master Key](../master-key/) — what rotation replaces
- [Storage](../storage/) — where the Keyring lives
- [Storage Object](../storage-object/) — the broader object category a
  Keyring belongs to
- [Specification register](../../spec/) — the behavioral rules cited by ID
