# Pack

## Definition

**Pack** is a [Container](../container/) whose [Entries](../container/entry/)
all belong to a single complete semantic unit — one scanned book, one
finished album. A Pack holds either the whole unit or, when the unit exceeds
the size target, one path-ordered slice of it.

The unit itself is represented by the Entry paths its files share — it has no
object of its own on [Storage](../storage/).

Packing keeps the number of objects on Storage small and hides how many
files each unit contains.

## Examples

- One scanned book (300 page images, ~1 GB): one Pack holding the whole book
- The album folder `albums/2023/` (hundreds of GB), packed once the year is
  over: a few hundred Packs, each holding a slice of roughly 1 GiB

## Collocations

- pack (a completed folder into Packs)
- repack (loose Containers into Packs)
- open (a unit by fetching its Packs)

## Domain Rules

- Only **complete** units are packed. A folder that still receives new files
  stays as one Container per file, and is packed when it is done.
- The size target (initially ~1 GiB) decides whether a unit fits in one Pack
  and where a larger unit is sliced.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.
- Because Containers are immutable, any change inside a Pack means
  re-uploading that Pack. Slicing caps this cost at the size target, not at
  the size of the whole unit.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
