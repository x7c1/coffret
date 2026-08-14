# Index Snapshot

## Definition

**Index Snapshot** is a control [Storage Object](../storage-object/) with two
purposes. It carries an encrypted copy of the [Index](../index/), so a new
or recovering device obtains a ready-made Index quickly instead of rebuilding
it by replaying the [Journal](../journal/) and opening every current
[Container](../container/). And it is the Journal's checkpoint: it records
everything recovery needs about the history it has applied, which is what
later makes deleting that history safe.

## Mental Model

An ordinary Index Snapshot summarizes the committed state up to one Journal
record: the control-head generation it represents, the last Journal
generation it applies, the committed [Keyring](../keyring/) commitment tuple
— Master Key epoch, generation, replica count, set digest — and the next
commit slot for its successor (spec: CK-1 to CK-3). The Journal records it
covers can then be pruned, because the Snapshot stands in for them.

The activation Snapshot of a new [Master Key](../master-key/) epoch does
more than summarize: it takes the current head's commit slot itself, fencing
writers still on the old epoch, and becomes the new head
(spec: CP-2, CP-6, MR-2).

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
- An Index Snapshot has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the
  Master Key, which breaks the recovery bootstrap dependency on the Keyring.

## Related Concepts

- [Index](../index/) — what a snapshot captures
- [Journal](../journal/) — what a snapshot checkpoints
- [Storage](../storage/) — where snapshots are kept
- [Storage Object](../storage-object/) — the broader object category a
  snapshot belongs to
