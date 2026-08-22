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
- A laptop that maps only `albums/` and a desktop that maps only `books/`
  hold the same Index of their shared Library; each scans its own subtree,
  and the laptop's Index still lists every page under `books/`

## Collocations

- rebuild (the Index from Storage)
- refresh (the Index after an upload)

## Domain Rules

- **The Index is a cache, never the source of truth.** A lost or corrupt
  Index does not lose Library data: it can be rebuilt exactly from Storage
  (spec: RV-5).
- **The Index catalogs the whole Library, not the part a device maps
  locally.** A device holding only `albums/` still knows which Container
  holds each page under `books/`; that is what lets every device restore the
  same Index from one [Index Snapshot](../index-snapshot/) (spec: CK-7,
  EP-9). Which of those Entries this device has actually placed on disk is
  device state kept beside the catalog, not part of it (spec: EP-10).
- A stale Index is brought forward from the newest Index Snapshot, replaying
  only the Journal records after it, so the Containers a device has to open
  are bounded by the commits since that Snapshot rather than since its own
  last sync (spec: CK-9).
- A rebuild is two-stage: the control state (defined in
  [Storage Object](../storage-object/)) determines which Containers are
  current, and opening those Containers then enumerates their Entries
  (spec: RV-1, RV-5). An [Index Snapshot](../index-snapshot/) short-cuts both
  stages with a ready-made Index.
- Container metadata says what a Container holds; only the control state
  says whether it is current, so a rebuild without that state yields salvage
  candidates rather than an accurate Index (spec: RV-4, RV-5).
- The Index also caches each Container's kind: the kind is recorded only
  inside the Container's encrypted meta section, and selecting `freeze`
  candidates needs it without opening Containers (spec: FM-9, PK-1, PK-15).

## Related Concepts

- [Storage](../storage/) — what the Index can be rebuilt from
- [Index Snapshot](../index-snapshot/) — an uploaded copy of the Index
- [Entry Path](../entry-path/) — the key of the cached mapping
- [Library](../library/) — what the Index catalogs
- [Specification register](../../spec/) — the behavioral rules cited by ID
