# Entry

## Definition

**Entry** is a single file stored inside a [Container](../), together with its
metadata: its [Entry Path](../../entry-path/) within the
[Library](../../library/), the modification time, and a hash of the file
content. The metadata is what lets a file keep its identity and be verified
inside an otherwise opaque Container.

## Examples

- `books/some-novel/page-042.png` stored as one Entry of a 300-entry
  [Pack](../../pack/)
- A single photo stored as the only Entry of its Container

## Collocations

- fetch (a single Entry from a Container) — without downloading the rest
- verify (an Entry against its recorded hash)

## Domain Rules

- An Entry is indivisible across Containers: a file larger than the Pack size
  target remains one Entry in one oversized singleton Container (spec: PK-3).
  - Indivisibility does not cost random access: a Container's ciphertext
    can be read and decrypted in ranges, so even a huge Entry streams
    without fetching the whole object.

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the Entry's canonical name in the Library
- [Pack](../../pack/) — a Container holding one path-ordered `freeze` segment
- [Library](../../library/) — where Entry Paths point back to
- [Specification register](../../../spec/) — the behavioral rules cited by ID
