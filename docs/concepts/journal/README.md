# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of how the set of
[Containers](../container/) changes over time. Each upload batch appends one
Journal record — a small control [Storage Object](../storage-object/) listing
the Containers the batch added (with their ciphertext hashes), the Containers
it removed, and the exact [Keyring](../keyring/) commitment selected by the
batch.

Replaying the Journal yields the current Container set. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

## Examples

- Replacing a Pack holding {a, b} with one holding {a} appends a record with
  the new Pack in additions and the old Pack in removals — which is exactly
  what records that b was deleted
- A batch interrupted before its Journal record leaves only uncommitted
  candidate Containers, which its creating device may remove once it proves
  the batch never committed; a batch interrupted after its record is finished
  by replaying that record

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (delete checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch: its additions and
  removals take effect exactly when the record is created, never partially
  (spec: CP-1).
- Writes are serialized at the Journal head: each head has one commit slot,
  and exactly one successor — an ordinary record, or the Index Snapshot that
  activates a new Master Key epoch — can take it
  (spec: CP-2 to CP-6).
- A conflict between concurrent writers is surfaced for explicit resolution,
  because silently picking a winner would lose one side's write without a
  trace (spec: CP-7,
  EP-7).
- Each commit selects the exact Keyring generation whose envelopes match the
  post-commit Container set; envelopes never travel in Journal records
  (spec: CP-8 to CP-11).
- A Journal record is opened with a key derived directly from the Master Key,
  so the record that commits a batch is readable before any Keyring
  (spec: CP-12).
- A committed removal is final for that Container ID; restoring the same
  contents creates a new Container
  (spec: CP-14).
- The Journal and its checkpoint determine which Containers make up the
  current Library; recovery replays a checkpoint plus the later records
  (spec: RV-1).
  - Losing that history never loses Container ciphertext, but recovery
    without it is salvage: current membership can no longer be proven
    (spec: RV-4).
- Deleting a Container that no reachable record mentions requires proof that
  its batch never committed; recovery alone never authorizes cleanup
  (spec: OC-1 to OC-5).
- `prune` bounds retained history by deleting Journal records already covered
  by an Index Snapshot checkpoint; it never deletes Containers or files
  (spec: CK-4 to CK-6).

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Entry Path](../entry-path/) — the identity used to detect write conflicts
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — owns the envelopes and is selected by a Journal
  commitment
- [Storage](../storage/) — where the Journal lives
