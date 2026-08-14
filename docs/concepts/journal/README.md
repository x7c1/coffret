# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of how the set of
[Containers](../container/) changes over time. Each upload batch appends one
Journal record — a small control [Storage Object](../storage-object/) listing
the Containers the batch added (with their ciphertext hashes), the Containers
it removed, and the exact [Keyring](../keyring/) commitment — Master Key
epoch, generation, replica count, set digest — selected by the batch.

Replaying the Journal yields the current Container set. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

## Mental Model

Each committed record becomes the Journal's **head**, and the head exposes
one **commit slot** — the single place where the next record can be created
(spec: CP-2).

A batch and its Journal record move through one lifecycle:

| Stage | Container set | Record |
| --- | --- | --- |
| preparing | additions exist only as uncommitted candidates | none yet — the batch can still be abandoned |
| committed | additions and removals are part of the current set | created; it is the new head and carries the next commit slot |
| checkpointed | unchanged | an [Index Snapshot](../index-snapshot/) has applied it |
| pruned | unchanged | deleted; the Snapshot has recorded its Keyring commitment and its commit slot (spec: CK-2, CK-3) |

The single slot is what serializes writers: every commit consumes the slot
of the head it started from, so of the writers starting from the same head
exactly one succeeds (spec: CP-3). Losing that race is mechanical — the
loser refreshes the head, rebases, and retries (spec: CP-4, EP-7); only when
both writers changed the same [Entry Path](../entry-path/) does the retry
surface an explicit conflict instead (spec: CP-7). The same slot is how a
[Master Key](../master-key/) epoch activation fences old-epoch writers
(spec: CP-3, CP-5).

## Examples

- Replacing a [Pack](../pack/) holding {a, b} with one holding {a} appends a
  record with the new Pack in additions and the old Pack in removals — which
  is exactly what records that b was deleted
- A batch interrupted before its Journal record leaves only uncommitted
  candidate Containers, which its creating device may remove once it proves
  the batch never committed; a batch interrupted after its record was created
  is completed by replaying that record

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch: its additions and
  removals take effect exactly when the record is created, never partially
  (spec: CP-1).
- Exactly one successor can take a head's commit slot: an ordinary record,
  or the Index Snapshot that activates a new Master Key epoch
  (spec: CP-2 to CP-6).
- A conflict between concurrent writers is surfaced for explicit resolution,
  because silently picking a winner would lose one side's write without a
  trace (spec: CP-7, EP-7).
- Each commit selects the exact Keyring generation whose entries match the
  post-commit Container set; [Key Envelopes](../key-envelope/) never travel
  in Journal records, because the committed Keyring is their single Storage
  home (spec: CP-8 to CP-11).
- A Journal record is opened with a key derived directly from the Master Key,
  so the record that commits a batch is readable before any Keyring
  (spec: CP-12).
- A committed removal is final for that Container ID; restoring the same
  contents creates a new Container
  (spec: CP-14).
- The Journal and its checkpoint determine which Containers make up the
  current [Library](../library/); recovery replays a checkpoint plus the
  later records (spec: RV-1).
  - Losing that history never loses Container ciphertext, but recovery
    without it degrades to **salvage**, the recovery mode without currency
    guarantees: decryptable contents can still be presented, but current
    membership can no longer be proven (spec: RV-4).
- Deleting a Container that no reachable record mentions requires proof that
  its batch never committed; recovery alone never authorizes cleanup
  (spec: OC-1 to OC-5).
- `prune` bounds retained history by deleting Journal records already covered
  by an Index Snapshot checkpoint, and only while the checkpointed Keyring
  replica set is complete (spec: CK-4 to CK-6); it never deletes Containers
  or files.

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Entry Path](../entry-path/) — the identity used to detect write conflicts
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — owns the envelopes and is selected by a Journal
  commitment
- [Storage](../storage/) — where the Journal lives
- [Specification register](../../spec/) — the behavioral rules cited by ID
