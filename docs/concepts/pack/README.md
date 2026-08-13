# Pack

## Definition

**Pack** is a [Container](../container/) that bundles all files of one
complete semantic unit — one scanned book, one finished album — as its
[Entries](../container/entry/).

Packing keeps the number of objects on [Storage](../storage/) small and hides
how many files each unit contains.

## Examples

- One scanned book (300 page images) uploaded as a single Pack
- The album folder `albums/2023/` packed once the year is over

## Collocations

- pack (a completed folder into a Pack)
- repack (loose Containers into a Pack)
- open (a Pack to browse its Entries)

## Domain Rules

- Only **complete** units are packed. A folder that still receives new files
  stays as one Container per file, and is packed when it is done.
- How files are grouped into Packs is a **pack policy** — a rule separate
  from the storage format that can change over time; existing data can be
  repacked under a new policy.
- Because Containers are immutable, any change inside a Pack means
  re-uploading the whole Pack.

## Related Concepts

- [Container](../container/) — a Pack is one
- [Entry](../container/entry/) — what a Pack bundles
