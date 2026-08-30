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
to the Library root, map folders to top-level prefixes, or combine both — for
example, keeping most of the Library on one disk and `albums/` on another. The
Library root is the root of the [Entry Path](../entry-path/) namespace; it does
not have to correspond to one folder on disk. Every
[Entry](../container/entry/) records its Entry Path relative to that root and
never a device path, so one Library restores onto whatever arrangement of
disks a device happens to have.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …
- One Library arranged differently on two devices: a laptop maps only
  `albums/`, a desktop only `books/`; each syncs its own subtree, and each
  one's [Index](../index/) still catalogs the whole Library
- A laptop that maps `albums/` but keeps only `albums/2026/08/` on disk: the
  rest of the album stays in the Library, untouched by the laptop's syncs

## Collocations

- scan (the Library for new or changed files)
- sync (the Library to Storage)
- restore (the current Library state from intact Storage control state)
- salvage (decryptable file contents when Storage control state is incomplete)
- freeze (eligible local files in a folder directly into [Packs](../pack/))
- survey (the files a freeze will pack)
- update (modified local files by replacing their current Containers)
- materialize (an Entry into a file in a mapped folder)
- spool (a Container's ciphertext to a local file before uploading it)
- settle (what an interrupted run left behind, before this one scans)
- stamp (the filesystem identity a mapped root stood on, during a scan)
- stamp (a fetched file with its Entry's own modification time)
- surface (a file a run reports rather than silently skips)
- fetch (a folder's files back onto this device) — the Library-side name for
  what the [Pack](../pack/) concept calls `open`: one folder's files arrive by
  fetching the distinct Packs that hold them

## Domain Rules

- One Library has one active [Master Key](../master-key/) epoch and one
  [Storage](../storage/) location.
- A Library is named on Storage by its **Library ID**, a random 64-bit value
  drawn when the Library is created: its objects live in one **app folder**
  called `coffret-<library id>`. The ID is independent of the Master Key, so a
  rotation never moves the Library, and it identifies nothing about the user or
  the files — it is what lets several Libraries share one Storage location and
  what a recovering device looks for (spec: FM-18).
- A local folder maps either to the Library root or to a top-level prefix. A
  device may have at most one root mapping, and each prefix maps to at most one
  folder. When both are present, a prefix mapping represents that part of the
  Library and the root mapping represents the rest. These mappings belong to
  the device, so another device may arrange the same Library differently
  (spec: EP-9).
- A scan reports an Entry as deleted locally only if this device itself had
  **materialized** it — uploaded or fetched it into a mapped folder — and it is
  gone. Entries the device never materialized, mapped or not, are outside its
  scope rather than missing, so holding part of a Library never removes or
  rewrites the rest (spec: EP-10).
  - A mapped root this device cannot vouch for — missing, or empty while
    standing on a filesystem other than the one recorded for it — is an
    **unavailable root**. Nothing under it is walked and no Entry under it is
    reported as deleted; the run reports the root itself, so an unplugged disk
    or an unmounted share reads as a root to reconnect rather than an emptied
    folder (spec: EP-12).
- Multiple enrolled devices may write to one Library. Writes are serialized
  at the [Journal](../journal/) commit point, so no device is the permanently
  designated writer (spec: CP-2).
- Scanning local folders only discovers local changes. The current Library
  state changes only when a Journal commit accepts them (spec: CP-1).
- A sync runs in stages — settle what an interrupted run left, scan the mapped
  folders, **spool** each new Container's ciphertext to a local file, upload it,
  and commit — and only the commit changes the current Library state. Everything
  before it is device-local work that an interrupted run leaves behind for the
  next one to settle (spec: CP-1, OC-2, OC-7).
- A fetch writes its temporary file inside a mapped folder, which is also a
  folder a scan walks, so coffret reserves a local filename prefix for those
  files and a scan passes over every local name carrying it (spec: EP-11).
  - The cost is that anything of the user's own carrying that prefix is not
    backed up — a file, or a folder and everything under it, since the scan
    stops at the name and never looks inside — which is the trade for a crash
    never inventing an Entry out of a partial fetch.
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
- A `freeze` refuses to pack a file that changed after the **survey** — the
  first pass, which measures each selected file and settles the Pack's entry
  table before a byte of content is written. A file whose length or content
  moved in between would land under a table that does not describe it, so the
  run stops instead, leaves the Pack in its spool for the next run to settle,
  and the file is simply eligible again next time
  (spec: PK-18, FM-2, FM-5, FM-9, OC-2).
- A scan surfaces every file needing `update` — changed locally, or held by
  a Container whose key was lost — because silently skipping one would make
  the user believe stale or unrecoverable content is backed up
  (spec: PK-14, PK-11).
- Each file a run surfaces is reported as a **finding**, which is not an error:
  the run still succeeds, and every later run reports the same finding until
  someone acts on it, so a file needing attention never falls out of view
  (spec: PK-14, EP-10, EP-11).
  - An unavailable root is a finding of the same kind, about a mapping rather
    than a file, so a successful run carrying one has scanned less of the
    Library than this device's mappings cover (spec: EP-12, PK-14).
- One `freeze` invocation selects among the files under the folders its request
  names, so an update-eligible file outside them is outside that invocation's
  scope rather than a file it silently passed over — that surfacing obligation
  covers the files the scan considered, and a run over another folder, or over
  the Library root, considers the rest (spec: PK-17, PK-14).

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Entry Path](../entry-path/) — a file's canonical name in the Library
- [Storage](../storage/) — where the encrypted Library lives
- [Index](../index/) — the local catalog of the Library
- [Specification register](../../spec/) — the behavioral rules cited by ID
