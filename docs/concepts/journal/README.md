# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of how the set of
[Containers](../container/) changes over time. Each upload batch appends one
Journal record, a small control [Storage Object](../storage-object/) listing
three things:

- the Containers the batch added, with their ciphertext hashes and the
  Entries each one holds
- the Containers it removed
- the exact [Keyring](../keyring/) commitment the batch selected

Replaying the Journal yields the current Container set. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

## Mental Model

Each committed record becomes the Library's **control head**, the position
the next writer commits from, and the head exposes one **commit slot** — the
single place where that head's successor can be created (spec: CP-2).

A batch and its Journal record move through one lifecycle:

| Stage | Container set | Record |
| --- | --- | --- |
| preparing | additions exist only as uncommitted candidates | none yet — the batch can still be abandoned |
| committed | additions and removals are part of the current set | created; it is the new head and carries the next commit slot, plus the slot where its own [Index Snapshot](../index-snapshot/) goes |
| checkpointed | unchanged | an [Index Snapshot](../index-snapshot/) has applied it |
| pruned | unchanged | deleted; the Snapshot has recorded its Keyring commitment and its commit slot (spec: CK-2, CK-3) |

The single slot is what serializes writers: every commit consumes the slot
of the head it started from, so of the writers starting from the same head
exactly one succeeds (spec: CP-3). Conflicting changes to the same
[Entry Path](../entry-path/) are surfaced instead of silently choosing a
winner (spec: CP-7). The same slot is how a [Master Key](../master-key/) epoch
activation fences old-epoch writers (spec: CP-3, CP-5).

A record and the activation [Index Snapshot](../index-snapshot/) that could
take its place are therefore stored under one name, the head position's, not
under a name of their own kind: two names would be two slots, and the fencing
would fence nobody (spec: FM-12).

## Examples

- Replacing a [Pack](../pack/) holding {a, b} with one holding {a} appends a
  record with the new Pack in additions and the old Pack in removals — which
  is exactly what records that b was deleted

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch: its additions and
  removals take effect exactly when the record is created, never partially
  (spec: CP-1).
- A record also reserves where its own checkpoint goes, so the Index
  Snapshot of a head has exactly one home on Storage, under a name of the
  checkpoint's own rather than the head's (spec: CK-10, FM-12).
- A record carries the Entries of the Containers it added, so a device
  replaying the Journal reads records and opens no Container; the
  Container's own meta section stays the authority on what it holds
  (spec: CP-11, CK-9).
- Each commit selects the exact Keyring generation whose mapping matches the
  post-commit Container set; [Key Envelopes](../key-envelope/) never travel
  in Journal records, because the committed Keyring is their single Storage
  home (spec: CP-8 to CP-11).
- A committed removal is final for that Container ID; restoring the same
  contents creates a new Container
  (spec: CP-14).
- The Journal and its checkpoint determine which Containers make up the
  current [Library](../library/); recovery replays a checkpoint plus the
  later records (spec: RV-1).
  - Losing that history never loses Container ciphertext, but recovery
    without it degrades to **salvage**, the recovery mode without currency
    guarantees: decryptable contents can still be presented, but nothing
    proves which Containers are current (spec: RV-4).

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Entry Path](../entry-path/) — the Library position used to detect write
  conflicts
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — owns the envelopes and is selected by a Journal
  commitment
- [Storage](../storage/) — where the Journal lives
- [Specification register](../../spec/) — the behavioral rules cited by ID
