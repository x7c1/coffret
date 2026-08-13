# Entry Path

## Definition

An **Entry Path** is the canonical, Library-relative name of one
[Entry](../container/entry/). It is a Unicode string normalized to NFC and
encoded as UTF-8, with `/` between components. Entry Paths form the logical
namespace of a [Library](../library/); they are not raw platform filesystem
paths.

For example, `books/some-novel/page-042.png` identifies one logical file no
matter which [Container](../container/) currently holds it.

## Collocations

- normalize (a local relative path into an Entry Path)
- compare (Entry Paths for equality or ordering)
- collide (when two local paths normalize to the same Entry Path)

## Domain Rules

- Every component must be valid Unicode. It is normalized to NFC and encoded
  as UTF-8. A local filename that is not valid UTF-8 is unsupported and causes
  the scan to report an error rather than skip or rename it.
- An Entry Path is non-empty and relative to the Library root. It has no empty,
  `.` or `..` component, no leading or trailing `/`, and no NUL. `/` is the
  only logical separator.
- Equality is exact equality of the canonical UTF-8 bytes and is
  case-sensitive. Ordering is lexicographic over those bytes, independent of
  locale. NFC does not merge case, width variants, or merely similar-looking
  characters.
- If distinct local paths normalize to the same Entry Path, the operation
  fails with a path collision. Coffret never silently selects one file or
  invents a different name. Likewise, a device that cannot materialize two
  distinct Entry Paths reports an explicit compatibility error.
- At every committed Library state, one Entry Path identifies at most one
  current Entry. Before a [Journal](../journal/) commit, coffret removes every
  Entry owned by the record's removals from the current path map, then inserts
  every Entry owned by its additions. The commit is rejected if an insertion
  finds an existing Entry Path or if the additions contain a duplicate.
- This invariant applies to the current path map, not to every Container
  physically present on Storage. An old Container and its replacement, or a
  current Container and an uncommitted orphan, may contain the same Entry Path
  while only one belongs to the current Library state.
- A writer that loses the Journal commit race rebases onto the new head and
  repeats the same uniqueness check. Two concurrent writes to one Entry Path
  therefore become an explicit conflict rather than last-write-wins.
- The prototype scans regular files only and does not follow symbolic links.
  A symbolic link does not create an Entry Path for its target.

## Related Concepts

- [Entry](../container/entry/) — the file identified by an Entry Path
- [Library](../library/) — the namespace in which an Entry Path is unique
- [Journal](../journal/) — serializes changes to the current path map
- [Pack](../pack/) — orders Entries by Entry Path
- [Index](../index/) — caches the mapping from Entry Path to Entry location
