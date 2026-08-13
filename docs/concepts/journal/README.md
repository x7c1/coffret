# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of how the set of
[Containers](../container/) changes over time. Each upload batch appends one
Journal record — a small control [Storage Object](../storage-object/) listing
the Containers the batch added (with their ciphertext hashes and
[Key Envelopes](../key-envelope/)) and the Containers it removed.

Replaying the Journal yields the current Container set. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

## Examples

- Replacing a Pack holding {a, b} with one holding {a} appends a record with
  the new Pack in additions and the old Pack in removals — which is exactly
  what records that b was deleted
- A batch interrupted before its Journal record leaves only uncommitted
  Containers, which recovery discards; one interrupted after it is finished
  by replaying the record

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (Journal records covered by a checkpoint)

## Domain Rules

- The Journal record is the **commit point** of a batch: Containers not
  recorded in any record are uncommitted and may be discarded (the local
  Library is the upload source, so nothing is lost); recorded removals not
  yet physically deleted are completed on recovery. Both directions are
  idempotent.
- The Journal decides which Containers are current; the content itself stays
  self-describing in the Containers. Losing the Journal therefore never
  loses file content — at worst, removed Containers resurrect and cleanup
  runs again.
- Journal additions carry each new Container's Key Envelope, so a batch
  commit records membership and keys atomically. Records are pruned only
  after a [Keyring](../keyring/) generation covers their envelopes.
- A Journal record has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the Master
  Key, so the record that commits a batch is readable before the later
  Keyring checkpoint. Its own ciphertext hash is therefore not part of its
  additions.
- An [Index Snapshot](../index-snapshot/) checkpoints the Journal: records at
  or before its generation can be pruned, and recovery replays only later
  entries.

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — consolidates the envelopes the Journal carries
- [Storage](../storage/) — where the Journal lives
