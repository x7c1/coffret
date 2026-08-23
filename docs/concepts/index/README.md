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
  each keep their own Index, and both catalog the whole Library: the laptop's
  Index lists every page under `books/` although the laptop scans only its
  albums

## Collocations

- rebuild (the Index from Storage)
- refresh (the Index after an upload)

## Domain Rules

- **The Index is a cache, never the source of truth.** A lost or corrupt
  Index does not lose Library data: it can be rebuilt exactly from Storage
  (spec: RV-5).
- **The Index catalogs the whole Library, not only what this device keeps on
  disk.** A device holding only `albums/` still knows which Container holds
  each page under `books/`, which is what lets every device restore an
  identical Index from one [Index Snapshot](../index-snapshot/) (spec: CK-7,
  EP-9).
  - This device's own state is kept beside the catalog rather than in it: how
    it maps the Library onto its local folders, which Entries it has actually
    placed on disk, and what it has spooled or not yet finished uploading.
    None of that is ever uploaded, which is why one Snapshot restores the same
    catalog everywhere (spec: EP-9, EP-10, CK-7, OC-2).
- A stale Index catches up from whichever is newer, itself or the newest
  Index Snapshot, and replays only the Journal records after that point —
  which carry what the Containers they added hold, so no Container is opened
  — and the checkpoint policy keeps that stretch near its threshold however
  long this device was away (spec: CK-8, CK-9).
- A rebuild replays control state (defined in
  [Storage Object](../storage-object/)): the checkpoint and the records after
  it say which Containers are current and which Entries each holds, so an
  exact rebuild opens no Container (spec: RV-1, RV-5).
- Container metadata says what a Container holds; only the control state
  says whether it is current, so a rebuild without that state yields salvage
  candidates rather than an accurate Index (spec: RV-4, RV-5).
- Beside the Entries, the Index keeps for each current Container what a device
  needs before opening it: its kind, its ciphertext hash and length, and,
  where one is known, [Storage](../storage/)'s own identifier for it, which
  spares a listing before a fetch. All of it is a copy of what the
  Journal record that added the Container carried, so selecting `freeze`
  candidates and fetching one open no Container (spec: FM-9, FM-15, CP-11,
  PK-1, PK-15).

## Related Concepts

- [Storage](../storage/) — what the Index can be rebuilt from
- [Index Snapshot](../index-snapshot/) — an uploaded copy of the Index
- [Entry Path](../entry-path/) — the key of the cached mapping
- [Library](../library/) — what the Index catalogs
- [Specification register](../../spec/) — the behavioral rules cited by ID
