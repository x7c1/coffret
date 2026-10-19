# Index

## Definition

**Index** is a device's local store of a [Library](../library/)'s cached
[Catalog](../catalog/) and the records of how that device holds and changes the
Library. It lets coffret browse names, locate stored Entries, and compare local
files without asking [Storage](../storage/) for every lookup.

## Mental Model

The Index keeps three kinds of information with different recovery properties:

| Part | What it records | After loss |
| --- | --- | --- |
| Cached Catalog | Current Containers, Entries with their current and captured paths and their entry numbers, and the checkpoint reached | Reconstruct from intact control state |
| Device-local records | Mappings, materializations, and pending work | Restore the Index from a backup of this device, or re-establish the records on this device |
| Held parcels | Which parcels of which Containers this device keeps the ciphertext of | Nothing to recover: the parcels are read from Storage again when next wanted |

A new device restores the Catalog and chooses its own mappings. Rebuilding
only the cache preserves the existing device-local records. Losing the whole
Index loses those local records too; Storage contains no copy of them
(spec: CK-7, EP-9, EP-10, OC-2).

The cached Catalog is as current as the device's last catch-up made it. An
Index, and its cached Catalog, is **stale** when that catch-up finished and the
Library's head has moved past it since, because another device committed.
Nothing on this device knows that until a catch-up, a read Storage refuses, or
a look at the names of the Library's heads shows it. Such a look reads no
object and changes nothing, so a stale Index answers as the Catalog stood when
it was last caught up until a catch-up brings it forward (spec: CK-9, FM-12).

A **held parcel** is a parcel of a Container's chunk sequence this device read
and keeps on its own disk, beside the spool, so that reading a page a second
time asks Storage for nothing. It is ciphertext kept for reading again rather
than a record of anything: losing every held parcel costs only reads the
provider can observe, never correctness. The Index records which parcels are
held, and it is still what names the files: a fetch finds a parcel by its row,
never by looking in the directory. A parcel is let go once every Entry it
covers that this device maps is present or witnessed absent, or by the
device's next fetch, catch-up, or deletion after its Container has left the
current set (spec: PK-19, PK-21). When a server opens the Library, before it
serves anything, any parcel file in the directory that no row names is
discarded; that sweep only removes what the Index does not name, and never adds
a row. A parcel the record says is held whose file is gone, or no longer
authenticates as that parcel, is **unheld**: it was not held after all. The
device reads the parcel whole from Storage again and says so, rather than in
silence, because that second read is one the provider observes (spec: PK-20,
PK-21).

For a current Entry, **present** means this device recorded materializing it
at its current path and has not witnessed its file go. **Remote** means every
other current Entry: never materialized here, or recorded absent. An Entry a
rename gave a new path is therefore remote until its file is relocated there:
the record at the old path still says its file is present, but that does not
make the Entry present (spec: EP-10). These states describe local
availability; key-lost describes whether the Container can be decrypted, and
locked describes whether the running Library holds its Master Key.

A materialization record names the Entry its file is a copy of by that
Entry's Container and entry number, never by the path the file stands at. The
Index keeps the name current as a replacement — an update, a freeze absorbing
the file, a rebuilt Pack — gives the Entry a new Container and number: a
record whose Entry a record replaces at the path the Entry stood at follows
the replacement, and one whose Entry an adopted Snapshot no longer holds
follows the Entry standing where its own stood, where that Entry's Container
is new to the device. An Entry that was current already is never taken for
the record's own, however it came to the record's path (spec: EP-10).

It also carries the content hash of the Entry the file was made to match, and
it can outlive that Entry. Once the Entry leaves the
Library, a record still saying present marks the file as departed, and the
hash it carries is what tells an unedited file from an edited one, since the
Entry's own hash leaves the cached Catalog with it. The device forgets the
record once the file has gone to the desktop's trash or from the disk; a
forgotten record is not an absent one, because it witnesses no deletion
(spec: EP-15).

The Index keeps each Entry's entry number beside its paths, which is how it
finds the Entry a replayed rename names (spec: FM-9, FM-15). A rename changes
an Entry's current path and leaves this device's file where it was placed, so
the materialization record stays with the file and records the Entry's new
path until the file is relocated to follow it. While it waits, the file is not
the Entry at its own path: a scan neither reports it as deleted or departed nor
carries it in as a new file (spec: FM-15, EP-10). A device that adopts a
Snapshot past the rename records the new path the same way. A sync or a fetch
relocates the file before it scans or places anything, and only once the file
is relocated does the record follow it to the new path; a relocation that is
refused or interrupted leaves the record where its file is, and the next run
finishes it (spec: EP-10). Once the Entry leaves the Library the wait ends,
and the file is judged where it stands like any other: departed where no
current Entry stands there (spec: EP-15). Wherever another Entry stands at the
path a record's file stands at, by a rename or an addition, the device forgets
the record instead, since the file is not a copy of that Entry
(spec: EP-10, EP-11).

A **spool** holds a new Container's ciphertext on this device. A **pending row**
is its local provenance: which batch created it, where its spool and uploaded
object are, whether spooling finished, and whether a commit may have been
attempted and, for its batch, at which commit slots. Settlement uses that
evidence to distinguish work it can reclaim, work whose interrupted refresh it
can complete, and work it must retain.

A row records its uploaded object only once the upload has answered, so a row
whose spool finished and that names no object is not proof that nothing of its
Container is on Storage: the run may have ended between the upload and the
record. Settlement therefore looks the Container up by the name its ID gives it
before it disposes of such a row (spec: OC-2).

| Settlement evidence | Action |
| --- | --- |
| An abandoned batch proven never attempted, or an attempted one whose every recorded commit slot holds another record | Dispose of its spool and of its Container on Storage — the object the row names, or the one found under the Container's name — then remove the row |
| A completed spool whose Container is current, or named by the record at a slot its batch attempted | Complete this device's materialization records |
| An attempted or unknown commit, without proof of abandonment | Retain the ciphertext and provenance |

Only a run with exclusive ownership of this device's pending work may settle
it, so another live producer's files cannot be reclaimed (spec: OC-2, OC-3,
OC-7). Completion needs nothing but a caught-up Index, so it also follows every
other catch-up that is free to take that ownership — a server's as it starts, a
fetch's — while reclaiming and retaining stay with settlement (spec: OC-7).
Precise spool transitions and cleanup conditions belong to the
[specification register](../../spec/orphan-cleanup/).

## Examples

- The Index lists every page under `books/` on a laptop that maps only
  `albums/`; opening a remote page is the step that reaches Storage.
- Rebuilding an older cache leaves the device's mappings and pending rows
  intact. A lost Index file requires those local records to be recovered
  separately.

## Collocations

- rebuild (the cached Catalog in an Index from Storage)
- refresh (the Index after a commit, including this device's local records)
- catch up (a stale Index to the available committed head)
- restore (the cached Catalog from an Index Snapshot)
- adopt (a checkpoint into an Index)
- announce (a spool by recording its pending row)
- mark (a recorded spool complete, a materialized file present or absent, or a
  materialized file to follow a rename of its Entry)
- carry over (a materialization record to the Entry that replaces its own)
- re-key (a materialization record to the path its file was relocated to,
  after the file is relocated, or to the path a person renamed its file to on
  disk, once a sync has carried that rename into the Library)
- forget (a materialization record whose Entry left the Library, once its file
  has gone to the desktop's trash or from the disk, or at once where another
  Entry holds the path its file stands at)
- complete (an interrupted commit's local records from its pending row, during
  a sync's settlement or right after any other catch-up that finds its
  Container current)
- hold (a parcel this device read, by recording it beside its ciphertext)
- let go (of a held parcel whose mapped Entries are all present or witnessed
  absent, or whose Container left the current set)
- discard (a parcel file no row names, as a server opens the Library, before it
  serves anything)
- dispose (of a proven abandoned spool and uploaded object)
- remove (a pending row, once its spool is discarded and nothing of its
  Container can be on Storage — completed, or disposed of with its object
  trashed or shown absent) — never *delete*, which is the Library's verb for
  Entries
- retain (pending work whose commit outcome is unknown)

## Domain Rules

- The cached Catalog is reconstructible from intact control state, while
  device-local records are unique to this device; rebuilding the former must
  preserve the latter (spec: RV-5, CK-7).
- Every Index caches the whole Catalog, so a device can browse Entries it has
  never materialized, including when Storage is unreachable (spec: CK-7).
- Catch-up starts from the newer of the device's cached state and a valid
  checkpoint, then replays later Journal records without opening Containers
  (spec: CK-9, CP-11).
  - The Index refuses a record at or below the head it stands at, changing
    nothing, and catch-up steps over that refusal. Two processes catching up
    one Index therefore converge on the head instead of one of them carrying
    the catalog back (spec: CK-9, CK-13, RV-5).
- A read from a stale Index answers what the Index lists, which may be an
  Entry's earlier version, while the Container holding it stays within reach:
  readable while it is an untrashed removal, and after trash through a handle
  this device already holds. A removal takes a Container out of the current
  set while its object stays on Storage, so a reader keeps an answer while
  another device commits (spec: OC-6, CP-1).
  - Staleness surfaces as a refusal only once the object is out of reach:
    trashed with no handle here naming it, or gone for good. The reader who
    catches up — a server when the explorer asks, or the one catch-up a
    served Entry fetch makes on such a refusal — sees the current version
    (spec: CK-9, EP-5).
- Concurrent readers and writers may share an Index; producers and settlement
  additionally coordinate ownership of pending work to preserve its provenance
  (spec: CK-13, OC-2).
- A failed trash retains the pending row needed to retry that cleanup, because
  removing the evidence would turn a proven abandonment into an unresolved
  suspected orphan (spec: OC-2, OC-3).
- Local materialization records let a scan distinguish a file that disappeared
  from one this device never held, preventing a partial local Library from
  deleting remote contents (spec: EP-10).

## Related Concepts

- [Catalog](../catalog/) — the shared state cached here
- [Index Snapshot](../index-snapshot/) — the checkpointed Catalog, without local records
- [Mapping](../mapping/) — how local folders represent the Library
- [Library](../library/) — the scope of one Index
