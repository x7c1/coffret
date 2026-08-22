# Entry Path

Rule prefix: `EP`. The canonical form of an Entry Path, how paths are
compared, how collisions are surfaced, how local roots map onto the namespace,
and how uniqueness is enforced at the Journal commit.

Concept background: [Entry Path](../../concepts/entry-path/),
[Entry](../../concepts/container/entry/).

## Rules

- **EP-1.** Every Entry Path component is valid Unicode, normalized to NFC
  and encoded as UTF-8. A local filename that is not valid UTF-8 is
  unsupported and causes the scan to report an error rather than skip or
  rename the file. *(Form: test)*
- **EP-2.** An Entry Path is non-empty and relative to the Library root. It
  has no empty, `.`, or `..` component, no leading or trailing `/`, and no
  NUL; `/` is the only logical separator. *(Form: test)*
- **EP-3.** Equality is exact equality of the canonical UTF-8 bytes and is
  case-sensitive; ordering is lexicographic over those bytes, independent of
  locale. NFC does not merge case, width variants, or merely similar-looking
  characters. *(Form: test)*
- **EP-4.** If distinct local paths normalize to the same Entry Path, the
  operation fails with a path collision; coffret never silently selects one
  file or invents a different name. Likewise, a device that cannot
  materialize two distinct Entry Paths reports an explicit compatibility
  error. *(Form: test)*
- **EP-5.** At every committed Library state, one Entry Path identifies at
  most one current Entry. The invariant covers the current path map, not
  every Container physically present on Storage: an old Container and its
  replacement, or a current Container and an uncommitted orphan, may contain
  the same Entry Path while only one belongs to the current state.
  *(Form: test)*
- **EP-6.** Before a Journal commit, coffret removes every Entry owned by the
  record's removals from the current path map, then inserts every Entry owned
  by its additions. The commit is rejected if an insertion finds an existing
  Entry Path or if the additions contain a duplicate. *(Form: test)*
- **EP-7.** A writer that loses the Journal commit race rebases onto the new
  head and repeats the same uniqueness check, so two concurrent writes to one
  Entry Path become an explicit conflict rather than last-write-wins (CP-7).
  *(Form: test)*
- **EP-8.** The prototype scans regular files only and does not follow
  symbolic links; a symbolic link does not create an Entry Path for its
  target. *(Form: test)*
- **EP-9.** A device maps each local root either to the Library root or to a
  top-level Entry Path component. It may have at most one Library-root mapping
  and at most one mapping for each top-level component. When both kinds are
  present, a top-level mapping represents that subtree and the Library-root
  mapping represents the remainder. An invalid top-level component is
  rejected before any scan runs. The mappings to local paths are device state
  and are never uploaded. *(Form: test)*
- **EP-10.** A scan covers exactly the subtrees the device's mappings
  represent (EP-9): each top-level mapping covers its component's subtree,
  and the Library-root mapping covers every path no top-level mapping claims.
  A current Entry outside every mapped subtree is out of the device's scope —
  it has no local counterpart to compare against — so the scan never reports
  it as modified, never selects it for `update` or `freeze`, and never
  proposes its removal. *(Form: test)*
  - A device that maps only `albums/` therefore leaves `books/` untouched in
    every commit it makes, while its Index still lists both (CK-7).
