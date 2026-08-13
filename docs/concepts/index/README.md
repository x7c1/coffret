# Index

## Definition

**Index** is a device-local catalog that maps the [Library](../library/)'s
files to the [Containers](../container/) and [Entries](../container/entry/)
that hold them. It is what lets coffret detect changed files quickly and find
the right Container to fetch without asking [Storage](../storage/).

## Examples

- After a sync, the Index knows that `books/some-novel/page-042.png` lives
  in a specific Pack at a specific offset, so opening the book needs no
  lookup on Storage

## Collocations

- rebuild (the Index from Storage)
- update (the Index after an upload)

## Domain Rules

- **The Index is a cache, never the source of truth.** A lost or corrupt
  Index is not a failure: it can always be rebuilt by scanning Storage and
  opening each Container.

## Related Concepts

- [Storage](../storage/) — what the Index can be rebuilt from
- [Index Snapshot](../index-snapshot/) — an uploaded copy of the Index
- [Library](../library/) — what the Index catalogs
