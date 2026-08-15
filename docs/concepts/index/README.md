# Index

## Definition

**Index** is a device-local catalog that maps the [Library](../library/)'s
[Entry Paths](../entry-path/) to the [Containers](../container/) and
[Entries](../container/entry/) that hold them. It is what lets coffret detect
changed files quickly and find the right Container to fetch without asking
[Storage](../storage/).

## Examples

- After a sync, the Index knows that `books/some-novel/page-042.png` lives
  in a specific [Pack](../pack/) at a specific offset, so opening the book
  needs no lookup on Storage

## Collocations

- rebuild (the Index from Storage)
- refresh (the Index after an upload)

## Domain Rules

- **The Index is a cache, never the source of truth.** A lost or corrupt
  Index does not lose Library data: it can be rebuilt exactly from Storage
  (spec: RV-5).
- A rebuild is two-stage: the control state (defined in
  [Storage Object](../storage-object/)) determines which Containers are
  current, and opening those Containers then enumerates their Entries
  (spec: RV-1, RV-5). An [Index Snapshot](../index-snapshot/) short-cuts both
  stages with a ready-made Index.
- Container metadata says what a Container holds; only the control state
  says whether it is current, so a rebuild without that state yields salvage
  candidates rather than an accurate Index (spec: RV-4, RV-5).

## Related Concepts

- [Storage](../storage/) — what the Index can be rebuilt from
- [Index Snapshot](../index-snapshot/) — an uploaded copy of the Index
- [Entry Path](../entry-path/) — the key of the cached mapping
- [Library](../library/) — what the Index catalogs
- [Specification register](../../spec/) — the behavioral rules cited by ID
