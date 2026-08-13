# Index Snapshot

## Definition

**Index Snapshot** is an encrypted copy of the [Index](../index/) uploaded to
[Storage](../storage/) as a [Container](../container/). It lets a new or
recovering device obtain a ready-made Index quickly, instead of rebuilding it
by opening every Container.

## Collocations

- upload (an Index Snapshot after an upload batch)
- restore (the Index from an Index Snapshot)

## Domain Rules

- A snapshot is expendable: a stale, missing, or corrupt Index Snapshot costs
  only rebuild time, never data.
- The Index Snapshot is the one object on Storage with a recognizable name,
  so that recovery can find it without help. Its identity being visible to
  the provider is an accepted leak.

## Related Concepts

- [Index](../index/) — what a snapshot captures
- [Storage](../storage/) — where snapshots are kept
- [Container](../container/) — the form a snapshot is stored in
