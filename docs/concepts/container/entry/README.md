# Entry

## Definition

**Entry** is a single file stored inside a [Container](../), together with its
metadata: its [Entry Path](../../entry-path/) within the
[Library](../../library/), the modification time, and a hash of the file
content.

## Examples

- `books/some-novel/page-042.png` stored as one Entry of a 300-entry Pack
- A single photo stored as the only Entry of its Container

## Collocations

- fetch (a single Entry from a Container) — without downloading the rest
- verify (an Entry against its recorded hash)

## Domain Rules

- An Entry is indivisible across Containers. A file larger than the Pack size
  target remains one Entry in one oversized singleton Container; encryption
  chunks support streaming and range access without changing that ownership.

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the Entry's canonical name in the Library
- [Pack](../../pack/) — a Container holding one path-ordered `freeze` segment
- [Library](../../library/) — where Entry paths point back to
