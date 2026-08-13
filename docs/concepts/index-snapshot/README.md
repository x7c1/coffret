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

- Before any Journal history it covers is pruned, a snapshot is expendable
  and can be rebuilt. Once `prune` deletes that history, the snapshot becomes
  the required baseline for a restore until a newer valid checkpoint
  supersedes it. Losing that baseline does not alter Container ciphertext but
  limits recovery to salvage rather than a restore.
- An Index Snapshot checkpoints the [Journal](../journal/): it records the
  Journal generation it reflects, so recovery replays only later entries and
  older entries become eligible for `prune`. The Journal's Keyring redundancy
  gate still has to pass before those entries are deleted.
- An Index Snapshot belongs to one Master Key epoch and identifies the
  complete Keyring checkpoint it depends on.
- The Index Snapshot is an object on Storage with a recognizable name, so
  that recovery can find it without help. Its identity being visible to the
  provider is an accepted leak.
- An Index Snapshot has no Container Key or Key Envelope. It is encrypted and
  authenticated directly with a purpose-specific key derived from the Master
  Key, which breaks the recovery bootstrap dependency on the Keyring.

## Related Concepts

- [Index](../index/) — what a snapshot captures
- [Journal](../journal/) — what a snapshot checkpoints
- [Storage](../storage/) — where snapshots are kept
- [Storage Object](../storage-object/) — the broader object category a
  snapshot belongs to
