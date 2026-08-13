# Pack

## Definition

**Pack** is a [Container](../container/) holding one path-ordered segment
created by a [Library](../library/) `freeze` operation. The operation takes
the folder files selected at that moment, sorts them by
[Entry](../container/entry/) path, and cuts them into segments no larger than
the size target (initially ~1 GiB) — each segment becomes one Pack.

Packs know nothing about books, albums, or series. A browsing unit is simply
a folder: because Entries are path-ordered, all files under a folder occupy
a contiguous run of Packs, and opening the folder means fetching that run.
A unit larger than the size target spans several Packs automatically; small
neighboring units share a Pack automatically.

Packing keeps the number of objects on [Storage](../storage/) small: within
one `freeze` invocation, units bundle and split regardless of their size, so a
batch adds about its own size divided by the size target — plus at most one
undersized tail Pack. With reasonably sized invocations, the object count
stays near the total size divided by the size target; many tiny invocations
accumulate small Packs until they are compacted. Packing also detaches
object boundaries from semantic boundaries, so the provider cannot map
objects to books or albums.

## Examples

- One scanned book (~1 GB): roughly one Pack
- The album folder `albums/2023/` (hundreds of GB): a few hundred Packs
- A comic series of 300 volumes (~100 MB each) passed to one `freeze`: a few dozen
  Packs, each holding some ten consecutive volumes — fetching one volume
  brings its neighbors along, which doubles as read-ahead. Invoking `freeze`
  one volume at a time would instead leave 300 small Packs until compaction
  merges them

## Collocations

- pack (files selected by `freeze` into Packs)
- repack (Packs after a deletion or a policy change)
- open (a folder by fetching the Packs overlapping its path range)

## Domain Rules

- Only files selected by a `freeze` invocation are packed. `freeze` does not
  persist a folder state: files added later stay as one Container per file
  until another invocation selects them.
- The size target decides where segments are cut. When there is slack, cuts
  prefer folder boundaries, so that deleting a whole folder rarely touches a
  Pack shared with its neighbors.
- Deleting a folder removes the Packs fully inside its path range and
  repacks the boundary Packs it shares with neighbors (at most two per
  contiguous run of segments) — the cost is capped by the size target per
  boundary Pack, not by the size of what is deleted.
- Because Containers are immutable, any change inside a Pack means
  re-uploading that Pack. Segmentation caps this cost at the size target.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
- [Library](../library/) — whose `freeze` operation creates Packs
- [Index](../index/) — maps a path range to the Packs overlapping it
