# Index Snapshot

## Definition

**Index Snapshot** is a control [Storage Object](../storage-object/) that
carries an encrypted copy of the [Index](../index/) and checkpoints the
[Journal](../journal/). The Index copy lets a new or recovering device start
quickly without replaying a long Journal. The checkpoint records what
recovery needs from
the history it has applied, which is what later makes deleting that history
safe.

An ordinary Index Snapshot simply captures an already committed state. The
**activation Snapshot** used during [Master Key](../master-key/) rotation
carries the same full checkpoint, and also acts as the transition to the new
epoch: it consumes the current commit slot, fences old-epoch writers, and
becomes the new head. Because it occupies a head position rather than
checkpointing one, it is a distinct kind of object, with a key of its own and
a name taken from the head chain rather than from the checkpoints (spec:
FM-11, FM-12, KD-4).

## Mental Model

Every Index Snapshot summarizes a committed Library state. The checkpoint
content shared by ordinary and activation Snapshots includes four things
(spec: CK-1, CK-2, CK-3, CP-6):

- the control-head generation the Snapshot represents
- the last Journal generation it applies
- the committed [Keyring](../keyring/) commitment
- the next commit slot, for its successor

The Index copy beside it lists the current Containers and every current Entry
with the Container holding it, in Container ID and [Entry Path](../entry-path/)
order, so one Library state has exactly one encoding whichever device wrote it
(spec: FM-16).

An ordinary Snapshot records an existing committed head. The Journal records
it covers can then be pruned, because the Snapshot stands in for them.

An activation Snapshot carries that same checkpoint, but consumes the current
head's commit slot itself. This both activates the new epoch and fences writers
still on the old one (spec: CP-2, CP-6, MR-2). It is stored under the head
position's name — the same one an ordinary [Journal](../journal/) record would
have taken — because the two compete for that one position (spec: FM-12).

## Examples

- An object of a few MB on Storage holding a recent Index; a new device
  downloads it, replays the handful of Journal records after it, and can
  browse the Library within minutes

## Collocations

- upload (an Index Snapshot when the checkpoint policy asks for one)
- restore (the Index from an Index Snapshot)

## Domain Rules

- Before any Journal history it covers is pruned, a snapshot is expendable
  and can be rebuilt. Once `prune` (deletion of covered Journal records;
  spec: CK-6) deletes that history, the snapshot becomes the required
  baseline for a restore until a newer valid checkpoint supersedes it
  (spec: RV-1).
  - Losing that baseline does not alter Container ciphertext, but it limits
    recovery to salvage rather than a restore (spec: RV-4).
- The records a Snapshot applies become deletable only behind the Keyring
  completeness gate: after `prune`, the Keyring replicas are the envelopes'
  only carriers and the Snapshot's recorded tuple is the only proof of their
  selection (spec: CK-4, CK-5).
- The Index Snapshot is an object on Storage with a recognizable name, so
  that recovery can find it without help. Its identity being visible to the
  provider is an accepted leak; what the provider still sees despite the
  encrypted, size-padded payload is listed under
  [Storage Object](../storage-object/).
  - Both kinds are checkpoint candidates: a recovery or a stale device looks
    for the newest valid checkpoint among the ordinary Snapshots and the
    activation Snapshots alike (spec: CK-9, RV-1).
- Which device wrote a Snapshot does not matter to the device reading it: a
  Snapshot holds the Index of the whole Library and nothing about the
  writer's local folders — not how it mapped the Library, not which files it
  keeps on disk — so a device laid out differently restores from it
  unchanged (spec: CK-7).
- A Snapshot is written when the Journal since the newest one has grown past
  the checkpoint policy's threshold, before `prune`, and at activation — not
  after every commit — so a commit pays for its own batch alone, and the
  stretch a device catching up replays stays near that threshold
  (spec: CK-8).
- Every Journal record reserves one place for a Snapshot of its head; at
  most one is written there, and only when the policy asks. Any device may
  write it: two devices writing it at once end with the same checkpoint
  rather than two rivals under one name (spec: CK-10, CK-11).
- An Index Snapshot has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the
  Master Key, which breaks the recovery bootstrap dependency on the Keyring.

## Related Concepts

- [Index](../index/) — what a snapshot captures
- [Journal](../journal/) — what a snapshot checkpoints
- [Storage](../storage/) — where snapshots are kept
- [Storage Object](../storage-object/) — the broader object category a
  snapshot belongs to
- [Specification register](../../spec/) — the behavioral rules cited by ID
