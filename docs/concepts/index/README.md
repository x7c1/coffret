# Index

## Definition

**Index** is a device's local store of a [Library](../library/)'s cached
[Catalog](../catalog/) and the records of how that device holds and changes the
Library. It lets coffret browse names, locate stored Entries, and compare local
files without asking [Storage](../storage/) for every lookup.

## Mental Model

The Index keeps two kinds of information with different recovery properties:

| Part | What it records | After loss |
| --- | --- | --- |
| Cached Catalog | Current Containers, Entries, and the checkpoint reached | Reconstruct from intact control state |
| Device-local records | Mappings, materializations, and pending work | Restore a device backup or re-establish the records on this device |

A new device restores the Catalog and chooses its own mappings. Rebuilding
only the cache preserves the existing device-local records. Losing the whole
Index loses those local records too; Storage contains no copy of them
(spec: CK-7, EP-9, EP-10, OC-2).

For a current Entry, **present** means this device recorded materializing it
and has not witnessed its file go. **Remote** means every other current Entry:
never materialized here, or recorded absent. These states describe local
availability; key-lost describes whether the Container can be decrypted, and
locked describes whether the running Library holds its Master Key.

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
- complete (an interrupted commit's local records from its pending row)
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
