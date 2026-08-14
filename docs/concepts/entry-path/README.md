# Entry Path

## Definition

An **Entry Path** is the canonical, Library-relative name of one
[Entry](../container/entry/). It is a Unicode string normalized to NFC and
encoded as UTF-8, with `/` between components. Entry Paths form the logical
namespace of a [Library](../library/); they are not raw platform filesystem
paths — without one canonical form, the same file would get different
identities on different platforms.

For example, `books/some-novel/page-042.png` identifies one logical file no
matter which [Container](../container/) currently holds it.

## Collocations

- normalize (a local relative path into an Entry Path)
- compare (Entry Paths for equality or ordering)
- collide (when two local paths normalize to the same Entry Path)

## Domain Rules

- An Entry Path has exactly one canonical byte form
  (spec: EP-1, EP-2).
- Equality is byte-exact and case-sensitive; ordering is lexicographic over
  the canonical bytes, independent of locale
  (spec: EP-3).
- A local file that cannot become its own Entry Path — invalid encoding, or a
  collision where two local paths normalize to one — is reported as an
  explicit error, because a silent skip or rename could hide one of the
  user's files (spec: EP-1, EP-4).
- In the current Library state, one Entry Path identifies at most one current
  Entry. The [Journal](../journal/) commit enforces this against the current
  path map — which Entries are live, not which Containers sit on Storage,
  where a replaced Container may still carry the same Entry Path
  (spec: EP-5, EP-6).
- Two concurrent writes to one Entry Path become an explicit conflict
  (spec: EP-7, CP-7).

## Related Concepts

- [Entry](../container/entry/) — the file identified by an Entry Path
- [Library](../library/) — the namespace in which an Entry Path is unique
- [Journal](../journal/) — serializes changes to the current path map
- [Pack](../pack/) — orders Entries by Entry Path
- [Index](../index/) — caches the mapping from Entry Path to Entry location
- [Specification register](../../spec/) — the behavioral rules cited by ID
