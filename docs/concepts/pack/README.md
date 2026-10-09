# Pack

## Definition

**Pack** is a [Container](../container/) explicitly classified as managed by
the pack policy. Packing operations — `freeze`, repack, and compaction — create
Packs from path-ordered segments of [Entries](../container/entry/). `update`
and deletion replace a Pack by **read-modify-replace**: they read every Entry
of the old Pack and write a new Container that carries the unchanged ones
forward, substitutes changed ones, and omits deleted ones (spec: PK-10). That
new Container is another Pack; Pack-ness survives the replacement, while
Container identity does not. A Pack is therefore a persistent Container kind,
not a lineage back to one `freeze` invocation (spec: PK-15).

`freeze` is the one-shot [Library](../library/) operation that packs
[eligible](#domain-rules) local files into new Packs (spec: PK-1). One
invocation selects the eligible files in a folder — or, where its request
names them, only those files of the folder — sorts them by
[Entry Path](../entry-path/), and cuts them into segments around a target
size. Each Pack it creates holds files from that invocation alone; a later
repack or compaction can create Packs that mix files from several
invocations. A request's *selection* — the Entry Paths a caller names — is not
what the invocation *selects*: it selects only the files of the selection that
are eligible, and naming a file makes it no more eligible (spec: PK-17).

Pack exists because Entry count alone says nothing about whether a
Container's contents are managed as a group, and the operations need an
explicit distinction:
`freeze` absorbs a file that was uploaded on its own and leaves a Pack
alone. Grouping also keeps the object count in a band where a pass over
every object on [Storage](../storage/) still finishes, and where a provider's
item and rate limits do not bite first. Two such passes are listing the
[app folder](../storage/#domain-rules), which is how a device without an
[Index](../index/) finds the control objects it rebuilds the
[Catalog](../catalog/) from, and opening every Container in
[salvage](../journal/#domain-rules). One object per file would put a 500-book
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
  — fetching one volume brings the parcels it overlaps, and with them
  whatever of the volumes on either side shares those parcels. Invoking
  `freeze` one volume at a time would instead leave 300 small Packs until
  compaction merges them
- A photographer who runs `freeze` every month, then runs it once over the
  whole year: none of the monthly Packs is touched. Only the files added
  since each monthly run are eligible, and they are re-sorted across the
  whole year, so the Packs built from them can straddle months

## Collocations

- pack (eligible local files selected by `freeze` into Packs)
- update (modified files by replacing their Containers without changing kind)
- delete (Entries of a Pack: the Pack is removed when none is left, and
  otherwise rebuilt by read-modify-replace around the ones it keeps)
- repack (Packs after a deletion or a policy change)
- open (a folder by reading, from the distinct Packs containing its current
  Entries, the parcels those Entries overlap; an Entry somebody asked for
  reachable by reading only the parcels it overlaps, ahead of the rest)

## Domain Rules

- A local file is eligible for `freeze` when it is new to the Library or when
  its current Entry is held by a one-file Container (the Container created
  when a single file was uploaded on its own). An Entry already in a Pack is
  never eligible: `freeze` neither reads existing Packs as input nor rewrites
  them, and only repack or compaction regroups them (spec: PK-1, PK-2).
- `freeze` persists no folder state: files added later are simply eligible
  for a later invocation (spec: PK-2).
- A drop added as a Pack packs the files that drop carried and nothing else:
  its `freeze` names them, so a one-file Container already in the destination
  folder is not absorbed by it, and how many Packs result is the target size's
  to decide rather than the folder's (spec: PK-17, PK-3).
- A `freeze` the person asks for on a folder packs every eligible file under
  it (spec: PK-17).
- A browsing unit is simply a folder: the [Index](../index/) resolves the
  folder's current [Entry Paths](../entry-path/) to the distinct Packs that
  contain them, and opening the folder means reading from that set the
  parcels its Entries overlap (spec: PK-16).
  - A reader wanting one page of an unfetched book does not wait for the
    gigabyte around it: only the parcels that page overlaps are read, and the
    page is released as soon as its own chunks have arrived. Those parcels stay
    on the device, so the pages beside it that share them are already there
    and are never asked of Storage again; the rest of the Pack, outside the
    parcels read, is still unfetched (spec: PK-16, PK-21).
- Deleting Entries examines every Pack holding one of them, however many that
  is, since Pack path ranges overlap (spec: PK-8, PK-9). A Pack left with
  nothing is removed; one that keeps Entries is rebuilt by read-modify-replace,
  which reads and verifies the whole old Pack and carries the kept Entries
  forward with what it recorded about them, in their order (spec: PK-10). A
  Pack that cannot be read — one whose key is lost among them — is never
  rebuilt with part of its Entries invented or dropped: the deletion is
  refused for that Pack and reported (spec: PK-10, KL-17).
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
