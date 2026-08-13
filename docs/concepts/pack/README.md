# Pack

## Definition

**Pack** is a [Container](../container/) holding one path-ordered segment of
the frozen part of the [Library](../library/): files the user has marked as
no longer changing are sorted by [Entry](../container/entry/) path and cut
into segments no larger than the size target (initially ~1 GiB) — each
segment becomes one Pack.

Packs know nothing about books, albums, or series. A browsing unit is simply
a folder: because Entries are path-ordered, all files under a folder occupy
a contiguous run of Packs, and opening the folder means fetching that run.
A unit larger than the size target spans several Packs automatically; small
neighboring units share a Pack automatically.

Packing keeps the number of objects on [Storage](../storage/) small — near
the total size divided by the size target, regardless of how large or small
the user's units are — and detaches object boundaries from semantic
boundaries, so the provider cannot map objects to books or albums.

## Examples

- One scanned book (~1 GB): roughly one Pack
- The album folder `albums/2023/` (hundreds of GB): a few hundred Packs
- A comic series of 300 volumes (~100 MB each): a few dozen Packs, each
  holding some ten consecutive volumes — fetching one volume brings its
  neighbors along, which doubles as read-ahead

## Collocations

- pack (frozen files into Packs)
- repack (Packs after a deletion or a policy change)
- open (a folder by fetching the Packs overlapping its path range)

## Domain Rules

- Only **frozen** files are packed. Files still being added or edited stay
  as one Container per file, and are packed once their folder is frozen.
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
- [Library](../library/) — whose frozen part the Packs tile
- [Index](../index/) — maps a path range to the Packs overlapping it
