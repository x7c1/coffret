# Pack

## Definition

**Pack** is a [Container](../container/) explicitly classified as managed by
the pack policy. Packing operations — `freeze`, repack, and compaction —
create Packs from path-ordered segments of [Entries](../container/entry/).
When `update` or deletion replaces a Pack by read-modify-replace, the new
Container is another Pack; Pack-ness survives the replacement, while
Container identity does not. A Pack is therefore a persistent Container kind,
not a lineage back to one `freeze` invocation (spec: PK-15).

`freeze` is the one-shot [Library](../library/) operation that packs eligible
local files into new Packs (spec: PK-1). One invocation selects the eligible
files in a folder, sorts them by [Entry Path](../entry-path/), and cuts them
into segments around a target size. Those particular Packs are local to that
invocation; later repack or compaction can create Packs spanning its boundary.

Pack exists because Entry count alone says nothing about whether a
Container's contents are managed as a group, and the operations need an
explicit distinction:
`freeze` absorbs a file that was uploaded on its own and leaves a Pack
alone. Grouping is also what keeps the object count in a band where
rebuilding without an [Index](../index/) — scanning every object on
[Storage](../storage/) — still finishes, and where a provider's item and
rate limits do not bite first; one object per file would put a 500-book
library past a hundred thousand.

## Mental Model

Two kinds of Container hold user data. The kind is explicit and cannot be
inferred from the Entry count (spec: PK-1, PK-15):

| Container kind | Entries | Created by | Replacement and regrouping |
| --- | --- | --- | --- |
| one-file Container | exactly one | uploading a single file on its own | `update` preserves the kind; `freeze` absorbs it into a Pack |
| Pack | one or more | `freeze`, repack, compaction | `update` and deletion preserve the kind; repack and compaction may regroup it |

The target is a pack-policy parameter, not a format constant, and not a hard
maximum. A **normal Pack** has one or more Entries and stays within the target
before padding. An [Entry](../container/entry/) larger than the target stays
indivisible and forms an **oversized singleton Pack**. The latter is a form of
Pack, not a third Container kind. A one-file Container and a singleton Pack
can each hold exactly one Entry, which is why the explicit kind — rather than
the Entry count — decides `freeze` eligibility.

## Examples

- One scanned book (~1 GB): one or a few Packs, depending on the configured
  target
- The album folder `albums/2023/` (hundreds of GB): many Packs
- A RAW image larger than the configured target: one oversized singleton
  Pack, without splitting the Entry across Containers
- A comic series of 300 volumes (~100 MB each) passed to one `freeze`: a few
  dozen Packs, each holding some ten consecutive volumes from that invocation
  — fetching one volume brings its neighbors along, which doubles as
  read-ahead. Invoking `freeze` one volume at a time would instead leave 300
  small Packs until compaction merges them
- A photographer who runs `freeze` every month, then runs it once over the
  whole year: none of the monthly Packs is touched. Only the files added
  since each monthly run are eligible, and they are re-sorted across the
  whole year, so the Packs built from them can straddle months

## Collocations

- pack (eligible local files selected by `freeze` into Packs)
- update (modified files by replacing their Containers without changing kind)
- repack (Packs after a deletion or a policy change)
- open (a folder by fetching the distinct Packs containing its current Entries)

## Domain Rules

- A local file is eligible for `freeze` when it is new to the Library or when
  its current Entry is held by a one-file Container (the Container created
  when a single file was uploaded on its own). An Entry already in a Pack is
  never eligible: `freeze` neither reads existing Packs as input nor rewrites
  them, and only repack or compaction regroups them (spec: PK-1, PK-2).
  - A modified or key-lost file still held by a one-file Container can take
    either path: `update` propagates its content, and `freeze` does too
    while also regrouping it into a Pack (spec: PK-13).
- `freeze` persists no folder state: files added later are simply eligible
  for a later invocation (spec: PK-2).
- One Journal batch commits a `freeze`: its additions are the new Packs, and
  its removals are only the one-file Containers those Packs replace; an
  initial import builds Packs directly from local files, with nothing to
  remove (spec: PK-7).
- A browsing unit is simply a folder: the [Index](../index/) resolves the
  folder's current [Entry Paths](../entry-path/) to the distinct Packs that
  contain them, and opening the folder means fetching that set.
  - An Entry occupies that path in the current state when the
    [Journal](../journal/) still holds its Container in the current set.
- `freeze` segmentation is local to one invocation, so Pack path ranges from
  different invocations may overlap or interleave; repack and compaction may
  later produce Packs spanning those invocation boundaries
  (spec: PK-3, PK-4, PK-8).
  - Within one invocation a unit larger than the target spans several Packs,
    and small neighboring units can share one; many tiny invocations instead
    leave small Packs until compaction merges them.
  - Grouping hides the per-file signal: page counts and individual file sizes
    stop showing up as object counts and object sizes.
  - How far object boundaries fall away from books and albums follows from
    how wide an invocation reaches: one spanning several works blurs the
    boundaries between them, while one confined to a single work yields Packs
    holding only that work.
- Because Containers are immutable, any change inside a Pack means
  re-uploading that Pack; `update` is the operation that does this,
  propagating modified files — and re-encrypting files whose Container lost
  its key — by read-modify-replace. The replacement is a new Container with
  Pack kind, not the same Pack object (spec: PK-11, PK-12, PK-15).
  The size target caps that cost except for an oversized singleton Pack,
  where the cost is the whole Entry (spec: PK-5, PK-6).
- Deleting a folder removes the Packs left with no retained Entry and
  replaces each **mixed Pack** — one holding both deleted and retained
  Entries — by read-modify-replace, which never commits a replacement it
  could not fully read back and verify. Even if one Entry remains, the
  replacement keeps Pack kind (spec: PK-9, PK-10, PK-15).
- Each operation keeps one job — `freeze` packs new files and one-file
  Containers, `update` propagates content changes, repack regroups after a
  deletion or policy change, and compaction regroups across invocations — so
  no operation silently does another's work.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
- [Entry Path](../entry-path/) — the canonical order used for segmentation
- [Journal](../journal/) — commits the replacement and retirement of the old
  Pack
- [Library](../library/) — whose packing operations create and regroup Packs
- [Index](../index/) — maps current Entry Paths to the Packs containing them
- [Specification register](../../spec/) — the behavioral rules cited by ID
