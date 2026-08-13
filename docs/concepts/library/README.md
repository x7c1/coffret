# Library

## Definition

**Library** is the complete set of files a user entrusts to coffret. On the
user's machine it is rooted at a single local folder (the library root);
every [Entry](../container/entry/) records its path relative to this root.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …

## Collocations

- scan (the Library for new or changed files)
- sync (the Library to Storage)
- restore (the Library from Storage)

## Domain Rules

- One Library corresponds to one [Master Key](../master-key/) and one
  [Storage](../storage/) location.
- The Library can always be restored in full from Storage plus the Master Key
  alone.

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Storage](../storage/) — where the encrypted Library lives
- [Index](../index/) — the local catalog of the Library
