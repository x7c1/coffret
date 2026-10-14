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
| Cached Catalog | Current Containers, Entries, and the checkpoint reached | Reconstruct from intact control state |
| Device-local records | Mappings, materializations, and pending work | Restore the Index from a backup of this device, or re-establish the records on this device |
| Held parcels | Which parcels of which Containers this device keeps the ciphertext of | Nothing to recover: the parcels are read from Storage again when next wanted |

A new device restores the Catalog and chooses its own mappings. Rebuilding
only the cache preserves the existing device-local records. Losing the whole
Index loses those local records too; Storage contains no copy of them
(spec: CK-7, EP-9, EP-10, OC-2).

The cached Catalog is as current as the device's last catch-up made it. An
Index, and its cached Catalog, is **stale** when that catch-up finished and the
Library's head has moved past it since, because another device committed.
Nothing on this device knows that until a catch-up, or a read Storage refuses,
shows it, so a stale Index answers as the Catalog stood when it was last caught
up (spec: CK-9).

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
and has not witnessed its file go. **Remote** means every other current Entry:
never materialized here, or recorded absent. These states describe local
availability; key-lost describes whether the Container can be decrypted, and
locked describes whether the running Library holds its Master Key.

A materialization record carries the content hash of the Entry the file was
made to match, and it can outlive that Entry. Once the Entry leaves the
Library, a record still saying present marks the file as departed, and the
hash it carries is what tells an unedited file from an edited one, since the
Entry's own hash leaves the cached Catalog with it. The device forgets the
record once the file has gone to the desktop's trash or from the disk; a
forgotten record is not an absent one, because it witnesses no deletion
(spec: EP-15).

A **spool** holds a new Container's ciphertext on this device. A **pending row**
is its local provenance: which batch created it, where its spool and uploaded
object are, whether spooling finished, and whether a commit may have been
attempted. Settlement uses that evidence to distinguish work it can reclaim,
work whose interrupted refresh it can complete, and work it must retain.

| Settlement evidence | Action |
| --- | --- |
| An abandoned batch proven never attempted | Dispose of its spool and uploaded object |
| A completed spool whose Container is current | Complete this device's materialization records |
| An attempted or unknown commit, without proof of abandonment | Retain the ciphertext and provenance |

Only a run with exclusive ownership of this device's pending work may settle
it, so another live producer's files cannot be reclaimed (spec: OC-2, OC-3,
OC-7). Precise spool transitions and cleanup conditions belong to the
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
- mark (a recorded spool complete, or a materialized file present or absent)
- forget (a materialization record whose Entry left the Library, once its file
  has gone to the desktop's trash or from the disk)
- complete (an interrupted commit's local records from its pending row)
- hold (a parcel this device read, by recording it beside its ciphertext)
- let go (of a held parcel whose mapped Entries are all present or witnessed
  absent, or whose Container left the current set)
- discard (a parcel file no row names, as a server opens the Library, before it
  serves anything)
- dispose (of a proven abandoned spool and uploaded object)
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
