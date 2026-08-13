# Entry

## Definition

**Entry** is a single file stored inside a [Container](../), together with
its metadata: the original path within the [Library](../../library/), the
modification time, and a hash of the file content.

## Examples

- `books/some-novel/page-042.png` stored as one Entry of a 300-entry Pack
- A single photo stored as the only Entry of its Container

## Collocations

- fetch (a single Entry from a Container) — without downloading the rest
- verify (an Entry against its recorded hash)

## Related Concepts

- [Container](../) — the encrypted object an Entry lives in
- [Pack](../../pack/) — a Container whose Entries form one complete unit
- [Library](../../library/) — where Entry paths point back to
