# Entry Path

Rule prefix: `EP`. The canonical form of an Entry Path, how paths are
compared, how collisions are surfaced, how local roots map onto the namespace,
what a scan may report about the Entries a device holds, what a fetch may place
into a mapped folder, and how uniqueness is enforced at the Journal commit.

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
- **EP-10.** A device's mappings (EP-9) only translate Entry Paths into
  local paths; they do not assert that every Entry under a mapped subtree is
  present on the device. A scan discovers new and modified files under the
  mapped folders, and it reports an Entry as deleted locally only when the
  device itself had materialized it — uploaded it, or fetched it into place
  — and the file is now gone. An Entry the device never materialized,
  whether or not a mapping covers it, is never reported as modified, never
  reported as deleted, never selected for `update` or `freeze`, never proposed
  for removal, and never used as the source of a replacement — a
  read-modify-replace (PK-10) built from a local file this device never
  materialized would carry forward bytes this device never held. Which Entries a
  device has materialized is device state in the Index and never part of an
  Index Snapshot (CK-7). *(Form: test)*
  - A device that maps `albums/` but has fetched only `albums/2026/08/`
    therefore holds a partial subtree without the rest counting as deleted;
    a device with no mapping under `books/` leaves it untouched the same way,
    while its Index still lists all of it.
- **EP-11.** A fetch places an Entry at a local path only where the device can
  vouch for what is there: either nothing at all, or this device's own
  materialization record (EP-10) agreeing with the file on disk. Any other
  state — a file this device never placed, or one whose record and disk state
  disagree — is surfaced as a conflict and never overwritten. An Entry whose
  absence this device already witnessed is not re-fetched either. A fetched file
  becomes visible at its final path only once it is fully verified: its
  Container authenticates and the Entry's plaintext hashes to what the current
  catalog records for it, and the bytes reach the destination directory as a
  temporary file that is then renamed into place, so no reader ever observes a
  partial or unverified file. Every Entry a fetch declines to place is reported
  with the reason it was declined, on the same no-silent-selection posture EP-4
  sets. *(Form: test)*
  - The two states the device can vouch for are exactly the two EP-10 admits: a
    path outside its scope, which it may claim by placing a file there, and one
    it materialized itself, whose file it may replace with the same Entry's
    current content. A file it did not place may be an unsynced source file, so
    overwriting it would destroy content the Library never held.
  - The temporary file is written inside a mapped folder, which is also a folder
    a scan walks, so coffret reserves a local filename prefix for it: a fetch
    gives its temporary files no other kind of name, and a scan passes over every
    local name carrying that prefix instead of reporting it as a file to back up
    (EP-1, EP-8). A run killed between the write and the rename therefore leaves
    nothing a later sync would commit as an Entry. The cost is that anything of
    the user's own carrying that prefix is not backed up — a file, or a folder
    and everything under it, since the scan stops at the name and never looks
    inside — which is the trade for a crash never inventing an Entry out of a
    partial fetch.
- **EP-12.** Reporting an Entry as deleted locally (EP-10) requires the mapped
  root it stands under to be *available*: the root directory exists, and the
  filesystem it stands on is the one recorded for that mapping (EP-9). A device
  records that filesystem's identity per mapping, stamped by the scan that first
  sees the root and re-stamped whenever a root holding files stands on a
  different one; the identity is device state and is never uploaded (CK-7). A
  mapping whose root is missing, or whose root holds nothing and stands on a
  filesystem other than the recorded one, is unavailable: nothing under it is
  walked, no Entry under it is reported as deleted locally, no file under it is
  selected for `update` or `freeze`, and the run reports the mapping and the
  reason rather than returning silently — an unplugged disk or an unmounted
  share must never read as the user having emptied the folder. The device's
  other mappings scan normally, and a top-level mapping that is unavailable
  still represents its subtree, so the Library-root mapping neither walks it nor
  infers deletions under it (EP-9). *(Form: test)*
  - A root that holds files and stands on a filesystem other than the recorded
    one is available and is re-stamped: a device number that moved across a
    reboot or a remount is not evidence that a folder went away. The two
    conditions are asymmetric deliberately — every way the comparison can be
    wrong ends in skipping deletion inference or in re-stamping, never in
    reporting a deletion the recorded identity would not have. The state this
    leaves for a person is a folder genuinely emptied whose filesystem identity
    also moved, and the gesture is recording the mapping again, which clears the
    identity so the next scan stamps what is there.
  - What the rule does not cover: a mount point whose underlying directory is
    not empty reads as available and is re-stamped, because there is nothing to
    distinguish it from a folder holding files. That is the behavior of a device
    with no recorded identity at all, so the rule never makes such a root worse.
