# Index Snapshot

## Definition

**Index Snapshot** is a control [Storage Object](../storage-object/) containing
an encrypted copy of the [Index](../index/). It lets a new or recovering
device obtain a ready-made Index quickly, instead of rebuilding it by opening
every [Container](../container/).

## Examples

- An object of a few MB on Storage holding the latest Index; a new device
  downloads it and can browse the Library within minutes instead of opening
  every Container

## Collocations

- upload (an Index Snapshot after an upload batch)
- restore (the Index from an Index Snapshot)

## Domain Rules

- Before any [Journal](../journal/) history it covers is pruned, a snapshot
  is expendable and can be rebuilt. Once `prune` (deletion of covered Journal
  records; spec: CK-6) deletes that history, the snapshot becomes the
  required baseline for a restore until a newer valid checkpoint supersedes
  it (spec: RV-1).
  - Losing that baseline does not alter Container ciphertext, but it limits
    recovery to salvage rather than a restore
    (spec: RV-4).
- An Index Snapshot checkpoints the Journal: it records the generations and
  the committed [Keyring](../keyring/) commitment tuple — Master Key epoch,
  generation, replica count, set digest — that recovery needs, and the
  records it applies become deletable only behind the Keyring completeness
  gate (spec: CK-1, CK-3 to CK-5).
- An Index Snapshot represents a control head — the object that determines
  the next commit slot (spec: CP-2) — and carries that slot for its
  successor (spec: CK-2).
- The Index Snapshot is an object on Storage with a recognizable name, so
  that recovery can find it without help. Its identity being visible to the
  provider is an accepted leak.
- An Index Snapshot has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the
  [Master Key](../master-key/), which breaks the recovery bootstrap
  dependency on the Keyring.

## Related Concepts

- [Index](../index/) — what a snapshot captures
- [Journal](../journal/) — what a snapshot checkpoints
- [Storage](../storage/) — where snapshots are kept
- [Storage Object](../storage-object/) — the broader object category a
  snapshot belongs to
