# Pack

## Definition

**Pack** is a [Container](../container/) holding one path-ordered segment
created by a [Library](../library/) `freeze` operation. The operation takes
the folder files selected at that moment, sorts them by
[Entry Path](../entry-path/), and cuts them into segments around a target size
— each segment becomes one Pack. The target is a pack-policy parameter, not a
format constant. Its initial value is not yet fixed; prototype measurements
will compare candidates including 1 GiB and 2 GiB. The target is not a hard
maximum: an Entry larger than it remains indivisible and forms an oversized
singleton Pack.

Packs know nothing about books, albums, or series. A browsing unit is simply
a folder: because Entries are path-ordered, all files under a folder occupy
a contiguous run of Packs, and opening the folder means fetching that run.
A unit larger than the size target spans several Packs automatically; small
neighboring units share a Pack automatically. This remains true for an album
whose total size exceeds the target; only an individual oversized Entry uses
the singleton exception.

Packing keeps the number of objects on [Storage](../storage/) small: within
one `freeze` invocation, units bundle and split regardless of their semantic
size. Because Entries are indivisible, there can be more than one undersized
Pack: for example, consecutive 600 MiB files do not share a 1 GiB Pack. The
precise invariant is that no two adjacent normal Packs from the invocation
can be merged without exceeding the target. Many tiny invocations can add
further undersized Packs until compaction. Packing also detaches object
boundaries from semantic boundaries, so the provider cannot map objects to
books or albums.

## Examples

- One scanned book (~1 GB): one or a few Packs, depending on the configured
  target
- The album folder `albums/2023/` (hundreds of GB): many Packs
- A RAW image larger than the configured target: one oversized singleton
  Pack, without splitting the Entry across Containers
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
- Entries are indivisible. In path order, the policy appends the next Entry
  while the resulting pre-padding Container footprint stays at or below the
  size target. If adding it would exceed the target, the current non-empty
  Pack closes first. An Entry that exceeds the target by itself forms a
  singleton Pack. No empty Pack is created.
- The target applies to the pre-padding Container footprint: Entry contents,
  canonical metadata, and framing. Authentication tags and Padmé padding can
  make the stored ciphertext somewhat larger than the target.
- Deleting a folder removes the Packs fully inside its path range and
  repacks the boundary Packs it shares with neighbors (at most two per
  contiguous run of segments). A normal boundary Pack is capped by the target;
  an oversized singleton costs that Entry's size instead.
- Because Containers are immutable, any change inside a Pack means
  re-uploading that Pack. Segmentation caps this cost at the size target except
  for an oversized singleton Entry.
- Replacing a Pack after an Entry change or deletion is a
  read-modify-replace operation. The writer reads and verifies every Entry in
  the old Pack, carries each unchanged Entry forward, substitutes each changed
  Entry, and omits each deleted Entry. If any old Entry cannot be read and
  verified, the writer must not commit the replacement. If no Entry remains,
  the old Pack is removed without creating an empty replacement Pack.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.
- The initial target is chosen from prototype measurements of upload and
  retrieval behavior, rewrite amplification, object count, and provider API
  overhead. 1 GiB and 2 GiB are candidates, not guarantees.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
- [Entry Path](../entry-path/) — the canonical order used for segmentation
- [Journal](../journal/) — commits the replacement and retirement of the old
  Pack
- [Library](../library/) — whose `freeze` operation creates Packs
- [Index](../index/) — maps a path range to the Packs overlapping it
