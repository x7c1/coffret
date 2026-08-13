# Pack

## Definition

**Pack** is a [Container](../container/) that bundles files of one complete
semantic unit — one scanned book, one finished album — as its
[Entries](../container/entry/).

A unit larger than the size target is split into several Packs, each holding
a path-ordered slice of the unit. The unit itself is represented by the Entry
paths its files share — it has no object of its own on
[Storage](../storage/).

Packing keeps the number of objects on Storage small and hides how many
files each unit contains.

## Examples

- One scanned book (300 page images, ~1 GB) uploaded as a single Pack
- The album folder `albums/2023/` (hundreds of GB), packed once the year is
  over into a few hundred Packs of roughly 1 GiB each

## Collocations

- pack (a completed folder into Packs)
- repack (loose Containers into Packs)
- open (a unit by fetching its Packs)

## Domain Rules

- Only **complete** units are packed. A folder that still receives new files
  stays as one Container per file, and is packed when it is done.
- A Pack never mixes files from different units. A unit either fits in one
  Pack or spans several; the size target (initially ~1 GiB) decides where it
  is sliced.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.
- Because Containers are immutable, any change inside a Pack means
  re-uploading that Pack. Slicing caps this cost at the size target, not at
  the size of the whole unit.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
