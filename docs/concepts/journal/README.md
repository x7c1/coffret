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
- A batch interrupted before its Journal record may leave candidate orphan
  Containers. The device that created the batch may remove them after it can
  prove that the batch did not commit. A batch interrupted after its record is
  finished by replaying that record

## Collocations

- append (a Journal record at the end of a batch)
- replay (the Journal to determine the current Containers)
- checkpoint (the Journal into an Index Snapshot)
- prune (delete checkpointed Journal records no longer needed for recovery)

## Domain Rules

- The Journal record is the **commit point** of a batch. Before it exists, the
  batch has not changed the current Container set; once it exists, its
  additions and removals are part of that set.
- The Journal and its checkpoint determine which Containers make up the
  current Library. Recovery reconstructs that set from a valid Index Snapshot
  checkpoint followed by every later Journal record, or from the complete
  unpruned Journal history.
- Reconstructing the current set authorizes restore, not garbage collection.
  A Container being outside that set is necessary but not sufficient evidence
  that it is an uncommitted orphan: Storage may be withholding the newer
  Journal record that made it current. Automatic cleanup requires positive
  local provenance that identifies the creating batch and proof that the batch
  did not commit. Abandoning the batch before any commit attempt, or finding an
  authenticated different writer's record in the attempted commit slot, is
  such proof; an empty, unavailable, or ambiguous slot is not.
- A candidate without that provenance is retained and may be reported for
  manual review, but recovery never deletes it merely because no reachable
  Journal record or checkpoint mentions it. If an available authenticated Key
  Envelope makes the candidate decryptable, coffret may present its
  authenticated contents in isolation. After warning that a withheld Journal
  record could still make the candidate current, coffret may let the user
  explicitly move it to trash.
- If the checkpoint or required Journal history is incomplete, recovery
  becomes salvage and performs no automatic cleanup.
- A Container ID removed by a committed Journal record is never added again;
  restoring the same contents creates a new Container with a new ID. This
  makes membership removal monotonic. Recorded removals not yet physically
  deleted may therefore be completed on recovery. Proven orphan cleanup and
  removal completion are both idempotent.
- Each authenticated control head determines one next commit slot. Exactly one
  successor may consume it: an ordinary Journal record, or the Index Snapshot
  that activates a new Master Key epoch. Both use conditional create against
  the same slot, so activation atomically fences writers that still hold the
  old epoch. Of operations that start from the same head, exactly one succeeds;
  a conflicting ordinary writer has not committed. If another Journal record
  won, the writer refreshes the head before reconciling and retrying. If an
  activation Snapshot won, the old-epoch writer stops until it is re-enrolled.
  A successful activation Snapshot carries the new epoch's next commit slot
  and becomes the head for later records.
- A commit conflict never selects a winner by timestamps or silently applies
  last-write-wins. If both sides changed the same
  [Entry Path](../entry-path/), the conflict requires explicit resolution
  before retrying.
- Before a Journal commit, coffret computes the post-commit Container set as
  `(current - removals) union additions`. It writes and read-back verifies a
  complete candidate [Keyring](../keyring/) replica set whose Container IDs
  exactly equal that set. The previously committed Keyring remains
  authoritative until the Journal commit, so the candidate excludes removed
  Containers without making the pre-commit state unreadable.
- A Journal record commits to the candidate Keyring's `master_key_epoch`,
  generation, replica count, and `set_digest`. The digest binds the canonical
  complete mapping from Container IDs to Key Envelopes. Successfully creating
  the Journal record commits the batch and selects that exact Keyring replica
  set in one state transition. A candidate with any different commitment is
  not selected, even if it has the same generation.
- Journal additions do not carry Key Envelopes. Membership is the Journal's
  responsibility; the committed Keyring is the only Storage representation of
  the keys needed to open the resulting current Container set.
- A Journal record has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the Master
  Key, so the record that commits a batch is readable independently of the
  Keyring replica set. Its own ciphertext hash is therefore not part of its
  additions.
- Every Journal record belongs to one Master Key epoch.
- An [Index Snapshot](../index-snapshot/) checkpoints the Journal: records at
  or before the last Journal generation it applies become eligible for
  `prune`. The operation may run only when the Snapshot preserves the exact
  committed Keyring tuple and that replica set is complete. Recovery starts
  from the Snapshot's control-head generation and replays its later Journal
  successors.
- `prune` is the formal operation name in documentation and code. It deletes
  only eligible Journal records from Storage; it never deletes Containers,
  Library entries, or Library files. Its purpose is to bound retained Journal
  history and recovery replay.

## Related Concepts

- [Container](../container/) — what Journal records add and remove
- [Entry Path](../entry-path/) — the identity used to detect write conflicts
- [Storage Object](../storage-object/) — the broader object category a
  Journal record belongs to
- [Index Snapshot](../index-snapshot/) — the Journal's checkpoint
- [Keyring](../keyring/) — owns the envelopes and is selected by a Journal
  commitment
- [Storage](../storage/) — where the Journal lives
