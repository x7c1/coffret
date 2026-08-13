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
- A batch interrupted before its Journal record leaves candidate orphan
  Containers. Recovery may discard them only after reconstructing the complete
  current set from a valid checkpoint and all later records; a batch
  interrupted after its record is finished by replaying that record

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (delete checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch. The current Container
  set is reconstructed from a valid Index Snapshot checkpoint and every later
  Journal record, or from the complete unpruned Journal history. A Container
  may be discarded as an uncommitted orphan only after that
  reconstruction succeeds and proves that the object is not in the current
  set. Merely being absent from the retained Journal records is never enough:
  after `prune`, a current Container may be reachable only through the
  checkpoint. If the required checkpoint or Journal history is incomplete,
  recovery becomes salvage and performs no automatic cleanup. Recorded
  removals not yet physically deleted are completed on recovery. Both cleanup
  directions are idempotent.
- Each committed Journal head determines one next commit slot. Commits use
  optimistic concurrency against that authenticated head: of the writers that
  start from the same head, exactly one may append the next record. A
  conflicting writer has not committed; it refreshes the head, reconciles its
  local changes, and retries.
- A commit conflict never selects a winner by timestamps or silently applies
  last-write-wins. If both sides changed the same Entry path, the conflict
  requires explicit resolution before retrying.
- The Journal decides which Containers are current; self-description says
  what a Container holds, not whether it is current. If a required Journal
  record is missing and no valid later Index Snapshot covers it, a restore
  is impossible. Recovery becomes salvage: decryptable removed, replaced, and
  uncommitted Containers may appear beside current ones and must not trigger
  automatic cleanup or mutation.
- Journal additions carry each new Container's Key Envelope, so a batch
  commit records membership and keys atomically. Before that commit, a
  complete [Keyring](../keyring/) replica set covering the additions must
  already have been written and verified; this ensures that making a
  Container current never creates a single-copy envelope window.
- A Journal record has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the Master
  Key, so the record that commits a batch is readable independently of the
  Keyring replica set. Its own ciphertext hash is therefore not part of its
  additions.
- Every Journal record belongs to one Master Key epoch.
- An [Index Snapshot](../index-snapshot/) checkpoints the Journal: records at
  or before its generation become eligible for `prune`. The operation may run
  only after a complete Keyring replica set covers every envelope still needed
  after that checkpoint. Recovery then replays only later records.
- `prune` is the formal operation name in documentation and code. It deletes
  only eligible Journal records from Storage; it never deletes Containers,
  Library entries, or Library files. Its purpose is to bound retained Journal
  history and recovery replay.

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — consolidates the envelopes the Journal carries
- [Storage](../storage/) — where the Journal lives
