# Entry

## Definition

**Entry** is one stored representation of a file inside a [Container](../),
together with its metadata: the [Entry Path](../../entry-path/) where it
participates in the [Library](../../library/), the modification time, and a
hash of the file content. Replacing a file creates a new Entry, even when the
new Entry occupies the same Entry Path. The metadata preserves the file's
Library name and lets its stored content be verified inside an otherwise
opaque Container.

## Examples

- `books/some-novel/page-042.png` stored as one Entry of a 300-entry
  [Pack](../../pack/)
- A single photo stored as the only Entry of its Container

## Collocations

- fetch (a single Entry from a Container) — for prefetch and resume, without
  downloading the rest
- verify (an Entry against its recorded hash)

## Domain Rules

- An Entry is indivisible across Containers: a file larger than the Pack size
  target remains one Entry in one oversized singleton Pack (spec: PK-3).
  - Indivisibility does not cost random access: a Container's ciphertext
    can be read and decrypted in ranges, so even a huge Entry streams
    without fetching the whole object.

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Entry Path](../../entry-path/) — the canonical Library position occupied
  by a current Entry
- [Pack](../../pack/) — a Container holding one path-ordered `freeze` segment
- [Library](../../library/) — where Entry Paths point back to
- [Specification register](../../../spec/) — the behavioral rules cited by ID
