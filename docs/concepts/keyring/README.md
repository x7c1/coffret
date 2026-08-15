# Keyring

## Definition

**Keyring** is the control [Storage Object](../storage-object/) that owns the
[Key Envelopes](../key-envelope/) needed to open all current
[Containers](../container/). It exists for two reasons. Envelopes must live
outside the Containers, so that rotating the [Master Key](../master-key/)
rewrites only megabytes of compact control objects instead of every
Container. And an envelope, unlike the [Index](../index/), cannot be rebuilt
from anything else, so one object must own the authoritative envelopes and
replicate them — otherwise a single lost object could silently make a
Container unreadable forever.

One logical Keyring **generation** is stored as a **replica set** of several
independently encrypted objects. Every replica carries the generation's
complete mapping: every current Container to its Key Envelope, or to an
explicit **key-lost marker** when no copy of that key survives
(spec: KL-7). So reading
needs just one valid committed replica — the count adds redundancy, never a
quorum (spec: KL-6). Every generation belongs to one Master Key epoch and
numbers the successive mappings within it.

Four values identify one replica set exactly. Together they are its
**Keyring commitment**:

- the Master Key epoch the generation belongs to
- the generation's number within that epoch
- the replica count the generation was written with
- the **set digest**: a short fixed-size fingerprint computed from the
  mapping, which comes out different if any pair in the mapping changes

The digest is what lets a commitment name the set's exact contents rather
than just its place in the numbering, so two candidates sharing a generation
are never confused.

## Mental Model

A replica is one independently encrypted object carrying a generation's
complete mapping. The element-level property:

- A replica is **valid** when it decrypts and authenticates and its metadata
  and payload are internally consistent
  (spec: KL-1).

A generation's replica set moves through one lifecycle
(spec: KL-2 to KL-5):

| State | Meaning | Entered when |
| --- | --- | --- |
| partial candidate | some replicas written; no commit selects the set | a writer starts preparing the generation |
| complete set | every declared replica is valid and they all agree | preparation and read-back verification finish |
| committed | a commit selected the set's exact Keyring commitment | the selecting commit succeeds |
| degraded | committed, but object loss left fewer valid replicas than its commitment requires | replicas are lost or corrupted |

A degraded set is repaired automatically by whichever device detects it —
its missing replicas rewritten until the full committed count is back —
before the next mutation (spec: KL-11, KL-13).

Commitment is a selection: an ordinary [Journal](../journal/) commit makes
it, and so does the activation of a new [Master Key](../master-key/) epoch,
which consumes the same commit slot
(spec: KL-3). Validity and completeness
describe only the objects themselves. That split is why a complete candidate
still needs a commit to matter, and why an interrupted upload leaves harmless
partial candidates rather than damaged Keyrings.

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

- A Key Envelope is **irreplaceable**: it cannot be rebuilt from a Container,
  the Journal, or the Master Key, so every generation is stored as several
  replicas — initially three
  (spec: KL-6, KL-8).
  - Replication is effective only within a generation: a newer generation's
    envelopes are protected only by its own replicas
    (spec: KL-9).
- The committed Keyring maps every current Container and no other, so every
  current Container either opens through its envelope or is visibly recorded
  as key-lost — never silently unreadable (spec: KL-7).
- Each Journal commit selects the exact generation whose mapping matches the
  post-commit Container set; because selection is part of the commit itself,
  that set and its keys can never disagree
  (spec: CP-8 to CP-11,
  KL-3, KL-4).
- Restore can proceed from any one committed valid replica; a degraded set is
  repaired to the full count before the next write, Master Key rotation, or
  `prune` — the deletion of checkpointed Journal history
  (spec: CK-4 to CK-6) — because writing
  on thin redundancy would gamble the only copies of irreplaceable keys
  (spec: KL-11).
  - Repair is automatic and never silent: whichever device detects the
    degraded set rewrites the missing replicas, and the loss and repair are
    surfaced as a health event, because repeated replica losses are a signal
    of provider unreliability or tampering the user must see
    (spec: KL-13 to KL-15).
  - If repair cannot complete, the gate stays closed: reads and restore
    continue from the surviving replicas, writes stay refused, and the
    failure is reported and retried (spec: KL-16).
- A complete replica set that no commit ever selected is a suspected orphan:
  orphanhood has to be proven, so its disposal follows the Journal's cleanup
  rules
  (spec: KL-12,
  OC-2 to OC-5).
- Journal records become prunable only once an Index Snapshot preserves the
  selected commitment and the selected Keyring generation's replica set is
  complete, because after `prune` the Keyring replicas are the only carriers
  of those envelopes
  (spec: CK-5,
  CP-11).
- Losing every object that carries a current Container's envelope loses
  access to that Container's content, even with the Master Key and the
  ciphertext — the accepted price of cheap rotation.
  - The replica count protects against object-level loss within one Storage
    account, not loss of the Storage account itself.
  - The loss is recorded, never silent: the affected Containers are
    enumerated and reported — with their Entry Paths, where readable control
    state allows — and after a rebuild they stay current, recorded with
    key-lost markers, present but locked (spec: RV-7). They are deleted only
    by a genuine committed removal, and an `update` from a surviving local file
    heals them (spec: KL-17, PK-11).
  - A device still holding authenticated local key material may rebuild a
    fresh Keyring generation — envelopes where its material reaches,
    key-lost markers for the rest — and later material can upgrade markers
    to envelopes through ordinary commits (spec: RV-8).
- On rotation, every old-epoch Keyring, Journal record, and Index Snapshot is
  permanently deleted rather than trashed, because old-epoch control objects
  are exactly what a leaked old Recovery Code could open
  (spec: MR-3).
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
- [Specification register](../../spec/) — the behavioral rules cited by ID
