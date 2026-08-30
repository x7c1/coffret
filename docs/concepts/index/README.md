# Index

## Definition

**Index** is a device-local catalog that maps the [Library](../library/)'s
[Entry Paths](../entry-path/) to the [Containers](../container/) and
[Entries](../container/entry/) that hold them. It is what lets coffret detect
changed files quickly and find the right Container to fetch without asking
[Storage](../storage/).

## Mental Model

### Spool states of a pending row

A **spool** is the local file holding a Container's ciphertext before it is
uploaded; the verb names writing it. A device announces a spool by writing its
pending row before the spool file exists, so the row's own state is what says
whether that file is a whole Container yet (spec: OC-2):

| State | The spool file | Object handle |
| --- | --- | --- |
| `Spooling` | announced; absent, partial, or whole but unrecorded | none — only a `Spooled` spool is ever uploaded |
| `Spooled` | a whole Container | recorded once its upload lands |

The only transition is `Spooling` to `Spooled`, made by the spool step that
finished the file. A run that dies before that transition leaves ciphertext
nothing can open, since the Container's key was never committed, which is why
the next run disposes of such a row rather than resuming it (spec: OC-2, OC-7).

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
- catch up (a stale Index to the Library's head)
- restore (the Index from an [Index Snapshot](../index-snapshot/))
- adopt (a checkpoint from an [Index Snapshot](../index-snapshot/))
- announce (a spool, by recording its pending row before the file exists)
- mark (one recorded fact: a spool `Spooled`, an Entry present or absent)
- complete (an interrupted run's bookkeeping from its pending row)
- dispose (an interrupted run's spool, the object if one was uploaded, and the
  pending row naming them) — the reclaiming half of a settle, as against
  completing the bookkeeping of a spool whose batch did commit

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
    it maps the Library onto its local folders, which filesystem each mapped
    root stood on when a scan last saw it, which Entries it has materialized —
    the record naming such an Entry *present* names that same act — and what it
    is spooling, has spooled, or has not yet finished uploading. None of that is
    ever uploaded, which is why every device restores the same catalog from one
    Snapshot (spec: EP-9, EP-10, EP-12, CK-7, OC-2).
  - A **pending row** is the device-local record of a Container this device is
    about to spool, has spooled, or has uploaded before any commit: the batch it
    belongs to, the spool file, whether that file is a whole Container yet, and
    where the object went if it went (spec: OC-2, OC-7). The register calls the
    testimony such a row gives *local provenance*: the same record under the
    name cleanup's rules use for it (spec: OC-2).
  - Because no Index Snapshot and no Journal record carries device state, that
    state cannot be rebuilt from Storage at all — which is why a pending row an
    interrupted run left is the only surviving record of what this device did,
    and the only way to complete the bookkeeping of a commit whose Index
    refresh failed (spec: CK-7, OC-7).
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
  Journal record that added the Container carried, or of what the
  [Index Snapshot](../index-snapshot/) this device restored from listed among
  its current Containers, so selecting `freeze` candidates and fetching one open
  no Container (spec: FM-9, FM-15, FM-16, CP-11, PK-1, PK-15).

## Related Concepts

- [Storage](../storage/) — what the Index can be rebuilt from
- [Index Snapshot](../index-snapshot/) — an uploaded copy of the Index
- [Entry Path](../entry-path/) — the key of the cached mapping
- [Library](../library/) — what the Index catalogs
- [Specification register](../../spec/) — the behavioral rules cited by ID
