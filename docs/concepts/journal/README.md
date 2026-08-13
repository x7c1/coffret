# Journal

## Definition

**Journal** is the record on [Storage](../storage/) of how the set of
[Containers](../container/) changes over time. Each upload batch appends one
Journal entry — a small Container listing the Containers the batch added
(with their ciphertext hashes) and the Containers it removed.

Replaying the Journal yields the current Container set. This is what makes
removal expressible: without it, a scan that finds an old Container and its
replacement could not tell whether a file missing from the replacement was
deleted or still lives in the old Container.

## Examples

- Replacing a Pack holding {a, b} with one holding {a} appends an entry with
  the new Pack in additions and the old Pack in removals — which is exactly
  what records that b was deleted
- A batch interrupted before its Journal entry leaves only uncommitted
  Containers, which recovery discards; one interrupted after it is finished
  by replaying the entry

## Collocations

- append (a Journal entry at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (Journal entries covered by a checkpoint)

## Domain Rules

- The Journal entry is the **commit point** of a batch: Containers not
  recorded in any entry are uncommitted and may be discarded (the local
  Library is the upload source, so nothing is lost); recorded removals not
  yet physically deleted are completed on recovery. Both directions are
  idempotent.
- The Journal decides which Containers are current; the content itself stays
  self-describing in the Containers. Losing the Journal therefore never
  loses file content — at worst, removed Containers resurrect and cleanup
  runs again.
- An [Index Snapshot](../index-snapshot/) checkpoints the Journal: entries at
  or before its generation can be pruned, and recovery replays only later
  entries.

## Related Concepts

- [Container](../container/) — what Journal entries add and remove (a
  Journal entry is itself a small Container)
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Storage](../storage/) — where the Journal lives
