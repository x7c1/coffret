# Index Snapshot

## Definition

**Index Snapshot** is a control [Storage Object](../storage-object/) that
carries an encrypted copy of the [Index](../index/) and checkpoints the
[Journal](../journal/). The Index copy lets a new or recovering device start
quickly without replaying the Journal and opening every current
[Container](../container/). The checkpoint records what recovery needs from
the history it has applied, which is what later makes deleting that history
safe.

An ordinary Index Snapshot simply captures an already committed state. The
**activation Snapshot** used during [Master Key](../master-key/) rotation is
the same kind of object and carries the same full checkpoint. It also acts as
the transition to the new epoch: it takes the current commit slot, fences
old-epoch writers, and becomes the new head.

## Mental Model

Every Index Snapshot summarizes a committed Library state. The checkpoint
content shared by ordinary and activation Snapshots includes four things
(spec: CK-1 to CK-3, CP-6):

- the control-head generation the Snapshot represents
- the last Journal generation it applies
- the committed [Keyring](../keyring/) commitment
- the next commit slot, for its successor

An ordinary Snapshot records an existing committed head. The Journal records
it covers can then be pruned, because the Snapshot stands in for them.

An activation Snapshot carries that same checkpoint, but takes the current
head's commit slot itself. This both activates the new epoch and fences writers
still on the old one (spec: CP-2, CP-6, MR-2).

## Examples

- An object of a few MB on Storage holding the latest Index; a new device
  downloads it and can browse the Library within minutes instead of opening
  every Container

## Collocations

- upload (an Index Snapshot after an upload batch)
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
  provider is an accepted leak.
- Which device wrote a Snapshot does not matter to the device reading it: a
  Snapshot holds the Index of the whole Library and nothing about the
  writer's local folders — not how it mapped the Library, not which files it
  keeps on disk — so a device laid out differently restores from it
  unchanged (spec: CK-7).
- Every commit is followed by a Snapshot of its result, written by the
  committing device, so a new or recovering device starts from the latest
  committed state rather than from an old checkpoint plus a long Journal
  replay (spec: CK-8).
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
