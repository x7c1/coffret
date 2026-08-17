# Entry Path

## Definition

An **Entry Path** is a canonical, Library-relative position in the logical
namespace of a [Library](../library/). It is a Unicode string normalized to
NFC and encoded as UTF-8, with `/` between components. It is not a raw
platform filesystem path — without one canonical form, the same Library
position would get different names on different platforms.

At each committed Library state, a position may be occupied by a current
[Entry](../container/entry/). Replacing a file's content or its
[Container](../container/) puts a new Entry at the same position; moving the
file removes the old position and adds the new one. For example,
`books/some-novel/page-042.png` keeps the same Entry Path when an updated page
replaces the Entry stored there.

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

- [Entry](../container/entry/) — the stored file representation that occupies
  an Entry Path in a committed Library state
- [Library](../library/) — the namespace in which an Entry Path is unique
- [Journal](../journal/) — serializes changes to the current path map
- [Pack](../pack/) — orders Entries by Entry Path
- [Index](../index/) — caches the mapping from Entry Path to Entry location
- [Specification register](../../spec/) — the behavioral rules cited by ID
