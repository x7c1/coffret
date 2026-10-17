# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of committed changes to the
[Catalog](../catalog/). A **batch** is the unit it
records: the changes one run prepares together and
commits all at once or not at all. Each batch appends one Journal record, a
small control [Storage Object](../storage-object/). The current format carries
four vocabularies:

- the Containers the batch added, with their ciphertext hashes and the
  Entries each one holds — each Entry under its current
  [Entry Path](../entry-path/) and, where it differs, the captured path its
  Container recorded (see [Catalog](../catalog/#domain-rules)), and in the
  order of its Container's entry table, so its position is its entry number
- the Containers it removed
- the Entries it renamed, each named by its Container ID and
  [entry number](../container/entry/#mental-model) and given a new current
  Entry Path — the old name is the Catalog's and is not repeated
- the exact [Keyring](../keyring/) commitment the batch selected

Replaying the Journal reconstructs the Catalog, including the current Container
set and its Entries. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

This encrypted control-object log is distinct from coffret's device-local
diagnostic events: those explain runtime operations, never commit state, and
follow the event privacy boundary (spec: EL-1, EL-5).

## Mental Model

Each committed record becomes the Library's **control head**, the position
the next writer commits from, and the head exposes one **commit slot** — the
single place where that head's successor can be created (spec: CP-2). The
head carries that slot in the shape the [Storage](../storage/) identifies
objects by: a pre-minted identifier where the Storage mints identifiers, and
the successor's name `head-<generation + 1>`, re-derived rather than stored,
where it names objects. A writer never reserves a slot of its own
(spec: CP-2, CP-3, CP-15). So that even a Library's first commit has a head
to start from, the first head is written when the Library is created:
`head-0`, a record that adds and removes nothing.

A batch and its Journal record move through one lifecycle:

| Stage | Container set | Record |
| --- | --- | --- |
| preparing | additions exist only as uncommitted candidates | none yet — the batch can still be abandoned |
| committed | additions and removals are part of the current set | created; it is the new head and carries the next commit slot, plus the snapshot slot where its own [Index Snapshot](../index-snapshot/) goes |
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

- prepare (a batch's Containers before any commit)
- append (a Journal record at the end of a batch)
- rebase (a batch onto a head newer than the one it was prepared against)
- take over (an Entry, by the addition that replaces it, at the path a rename
  gave it meanwhile)
- replay (the Journal to determine the current Containers)
- consume (a commit slot, by creating the successor it admits)
- checkpoint (the Journal into an Index Snapshot)
- prune (checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch: its additions,
  removals and renames take effect exactly when the record is created, never
  partially (spec: CP-1).
- A record also reserves where its own checkpoint goes, so the Index
  Snapshot of a head has exactly one home on Storage, under a name of the
  checkpoint's own rather than the head's (spec: CK-10, FM-12).
- A record's name is recognizable, so recovery finds the head chain before any
  Index exists (spec: FM-12). What the provider still sees despite the
  encrypted, size-padded payload is listed under
  [Storage Object](../storage-object/).
- A record carries the Entries of the Containers it added, so a device
  replaying the Journal reads records and opens no Container; the
  Container's own meta section stays the authority on what it holds
  (spec: CP-11, CK-9, FM-15).
  - The record's byte form lists its additions and removals in Container ID
    order, and its renames in Container ID and then entry number order, so one
    committed state has exactly one encoding whichever device wrote it
    (spec: FM-15).
- Each commit selects the exact Keyring generation whose key table matches the
  post-commit Container set; [Key Envelopes](../key-envelope/) never travel
  in Journal records, because the committed Keyring is their single Storage
  home (spec: CP-8, CP-9, CP-10, CP-11).
- A committed removal is final for that Container ID; restoring the same
  contents creates a new Container
  (spec: CP-14).
- A batch commits only while every Container in its removals is still
  current, so a committed removal is never undone by a later batch prepared
  before it (spec: CP-14, CP-18).
- The Journal and its checkpoint determine which Containers make up the
  current [Library](../library/); recovery replays a checkpoint plus the
  later records (spec: RV-1).
  - Losing that history never loses Container ciphertext, but recovery
    without it degrades to **salvage**, the recovery mode without currency
    guarantees: decryptable contents can still be presented, but nothing
    proves which Containers are current (spec: RV-4).
- `prune` never leaves a Library that has committed anything with neither a
  head nor an [Index Snapshot](../index-snapshot/): it deletes only the
  records a checkpoint has applied, and never the Snapshot that applied them,
  which stays the source of the next commit slot once the last of those heads
  is gone (spec: CK-2, CK-4, CK-6).
  - No particular head is certain to survive — the first head is among the
    first records to become eligible for `prune`, and every head is eligible
    once a Snapshot covers the latest — so whoever asks whether a place holds
    a Library asks for any head or Snapshot, never for one by name
    (spec: CK-4, CK-6).

## Technical Constraints

**A rename is one record, and replay applies it.** A record's renames
take effect together, after its removals and before its additions, and the
state the whole record leaves keeps one Entry per Entry Path, so two Entries
can swap names in one record. A record one of whose renames names no current
Entry, changes nothing, names a derived Entry, or would leave two Entries at
one Entry Path is refused whole, and the Catalog stays as it was
(spec: FM-15, RV-5). A folder rename enumerates the current Entries being
renamed; replay does not discover additional Entries by interpreting a prefix.
Such a batch changes names without replacing or uploading Container
ciphertext. A record that carries renames also names the schema a reader
needs to apply it faithfully, and a device whose build implements less refuses
the record and stops its replay there, rather than skipping it or applying the
rest of it without the renames (spec: FM-9, FM-15).

**Rebase carries a batch over renames.** A batch remembers the head it was
prepared against, and every commit of it examines the records committed
since and applies their renames to its pending changes by Container ID and
entry number (spec: CP-4, EP-7). A content update, a [Pack](../pack/)
absorbing a file, or a Pack rebuilt by a deletion takes over the Entry at
the path another device's rename gave it, rather than bringing the old name
back; its Container still records the name it was written under. A batch
renaming an Entry that another device then replaced follows the replacement
where the name it was prepared against now names it. Two writes to one
Entry or one Entry Path — a rename onto the path an addition wants, two
renames of one Entry, a rename of an Entry replaced under another name or
removed, a rename onto a path another device filled — are refused as a
conflict naming the change, the record and the rule, never resolved by
picking one (spec: CP-7). Removals are carried over as they are, since they
name Containers, and a folder rename is the Entries it was prepared with and
no more.

Fetching finds an Entry by its entry number (spec: EP-11), and a device that
catches a rename up relocates its file of the renamed Entry before its next
sync or fetch scans or places anything — the device that made the rename,
right after the commit; a relocation it cannot make keeps the old
materialization record and stays pending for a later run (spec: EP-10).

## Related Concepts

- [Catalog](../catalog/) — the state reconstructed by replay
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
