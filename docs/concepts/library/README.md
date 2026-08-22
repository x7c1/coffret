# Library

## Definition

**Library** is a set of files a user entrusts to coffret, and the unit
everything else is scoped to: keys, Storage, and restore all operate on one
Library. A user may keep more than one — say one per Storage location — and
separate Libraries share nothing: their own Master Keys, Recovery Codes, and
Indexes.

The **current Library state** is the latest state accepted by a successful
[Journal](../journal/) commit. Local folders are a device's working view of
that state, not a second source of truth. They may temporarily differ from it:
for example, editing a local file creates a local change, and that change does
not become part of the current Library state until a sync commits it.

One or more local folders form that working view. A device can map one folder
to the library root, map folders to top-level prefixes, or combine both — for
example, keeping most of the Library on one disk and `albums/` on another. The
library root is the root of the [Entry Path](../entry-path/) namespace; it does
not have to correspond to one folder on disk. Every
[Entry](../container/entry/) records its Entry Path relative to that root and
never a device path, so one Library restores onto whatever arrangement of
disks a device happens to have.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …
- One Library arranged differently on two devices: a laptop maps only
  `albums/`, a desktop only `books/`; each syncs its own subtree and both
  share one [Index](../index/)

## Collocations

- scan (the Library for new or changed files)
- sync (the Library to Storage)
- restore (the current Library state from intact Storage control state)
- salvage (decryptable file contents when Storage control state is incomplete)
- freeze (eligible local files in a folder directly into [Packs](../pack/))
- update (modified local files by replacing their current Containers)

## Domain Rules

- One Library has one active [Master Key](../master-key/) epoch and one
  [Storage](../storage/) location.
- A local folder maps either to the library root or to a top-level prefix. A
  device may have at most one root mapping, and each prefix maps to at most one
  folder. When both are present, a prefix mapping represents that part of the
  Library and the root mapping represents the rest. These mappings belong to
  the device, so another device may arrange the same Library differently
  (spec: EP-9).
- A scan stays within the subtrees the device maps; Entries outside them are
  out of that device's scope, not missing, so a device that maps only part of
  the Library never removes or rewrites the rest (spec: EP-10).
- Multiple enrolled devices may write to one Library. Writes are serialized
  at the [Journal](../journal/) commit point, so no device is the permanently
  designated writer (spec: CP-2).
- Scanning local folders only discovers local changes. The current Library
  state changes only when a Journal commit accepts them (spec: CP-1).
- The Library's current Container set can be restored from the Master Key and
  Storage while the required control state (defined in
  [Storage Object](../storage-object/)) remains intact. A restore brings back
  exactly the Containers that were current, committed removals and
  replacements included; opening every current Container additionally
  requires its reachable Key Envelope, while a key-lost Container remains
  present but locked (spec: RV-1, RV-2, RV-7).
- If required Journal history or its [Index Snapshot](../index-snapshot/)
  checkpoint is missing, coffret can salvage contents from decryptable
  [Containers](../container/) but cannot prove which candidates are current;
  salvage is not a restore (spec: RV-4).
- `freeze` is a one-time packing operation, not a persistent folder state: it
  leaves no `frozen` flag to restore, and files added later simply become
  eligible for a later invocation
  (spec: PK-1, PK-2, PK-7).
- A scan surfaces every file needing `update` — changed locally, or held by
  a Container whose key was lost — because silently skipping one would make
  the user believe stale or unrecoverable content is backed up
  (spec: PK-14, PK-11).

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Entry Path](../entry-path/) — a file's canonical name in the Library
- [Storage](../storage/) — where the encrypted Library lives
- [Index](../index/) — the local catalog of the Library
- [Specification register](../../spec/) — the behavioral rules cited by ID
