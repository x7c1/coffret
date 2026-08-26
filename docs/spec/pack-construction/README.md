# Pack Construction

Rule prefix: `PK`. What makes a Container a Pack, which files `freeze`
selects, how it cuts them into Packs, what its Journal batch contains, how
`update` propagates modified files, and how Packs are replaced or removed
when Entries change or are deleted.

Concept background: [Pack](../../concepts/pack/),
[Library](../../concepts/library/), [Entry](../../concepts/container/entry/).

## Rules

- **PK-1.** A local file is eligible for `freeze` when it is not yet in the
  Library or when its current Entry is held by a one-file Container. A
  current Entry held by a Pack is not eligible: existing Packs are never
  inputs to `freeze` and are never rewritten by it. *(Form: test)*
- **PK-2.** `freeze` persists no folder state: files added later become
  eligible for a later invocation, and running it again leaves every existing
  Pack byte-for-byte unchanged. *(Form: test)*
- **PK-3.** Segmentation sorts the selected Entries by Entry Path and, in
  that order, appends the next Entry while the resulting pre-padding
  Container footprint stays at or below the size target; if adding it would
  exceed the target, the current non-empty Pack closes first. *(Form: test)*
  - An Entry that exceeds the target by itself forms an oversized singleton
    Pack — Entries are indivisible across Containers.
  - No empty Pack is created.
- **PK-4.** The resulting invariant within one invocation: no two adjacent
  normal Packs can be merged without exceeding the target — because Entries
  are indivisible, more than one undersized Pack can result (consecutive
  600 MiB files do not share a 1 GiB Pack). *(Form: test)*
- **PK-5.** The size target is a pack-policy parameter, not a format
  constant. Its initial value is chosen from prototype measurements of upload
  and retrieval behavior, rewrite amplification, object count, and provider
  API overhead; 1 GiB and 2 GiB are candidates, not guarantees. *(Form: test
  for the parameter mechanism; the value choice itself is a design decision
  recorded outside this register)*
- **PK-6.** The target applies to the pre-padding Container footprint: Entry
  contents, canonical metadata, and framing. Authentication tags and Padmé
  padding can make the stored ciphertext somewhat larger than the target.
  *(Form: test)*
- **PK-7.** One Journal batch commits a `freeze`: its additions are the new
  Packs, and its removals are only the eligible one-file Containers those
  Packs replace — newly imported files have no removal, and existing Packs
  never appear in removals. On an initial import, `freeze` builds Packs
  directly from local files without first uploading one-file Containers.
  *(Form: test)*
- **PK-8.** Path ordering, adjacency, and segmentation are local to the
  Entries selected by one `freeze` invocation: Packs do not form one
  non-overlapping partition of the Library's Entry Path order, and path
  ranges of Packs from different invocations may overlap or interleave.
  Producing that grouping across invocations is the job of the separate
  repack or compaction operation. *(Form: test)*
- **PK-9.** Deleting a folder examines every current Pack containing an Entry
  under that folder: a Pack whose Entries are all deleted is removed, and a
  Pack that also contains retained Entries is replaced by
  read-modify-replace (PK-10). *(Form: test)*
  - Because Pack path ranges can overlap across invocations, the number of
    mixed Packs is not bounded by two.
  - Under the initial policy every mixed Pack is normal, with its pre-padding
    footprint capped by the target; an oversized singleton cannot be mixed
    because it contains only one Entry.
- **PK-10.** Read-modify-replace reads and verifies every Entry in the old
  Pack, carries each unchanged Entry forward, substitutes each changed Entry,
  and omits each deleted Entry. If any old Entry cannot be read and verified,
  the writer must not commit the replacement; if no Entry remains, the old
  Pack is removed without creating an empty replacement. *(Form: test)*
- **PK-11.** `update` is the operation that propagates local content
  modifications into Storage. A local file is eligible for `update` when it
  is already in the Library and its local content differs from its current
  Entry, or when its current Entry's Container carries a key-lost marker
  (KL-7) regardless of content equality — the stored ciphertext is
  unreadable under a lost key, so re-encrypting the local plaintext into a
  replacement Container is the only content-recovery path. *(Form: test)*
  - When cached key material for the Container survives, upgrading the
    marker to an envelope (RV-8) is the lighter recovery — a Keyring-only
    write; `update` is the path when only the plaintext survives.
- **PK-12.** `update` replaces each Container holding a modified Entry by
  read-modify-replace (PK-10) and commits every swap through one Journal
  batch: the replaced Containers in removals, their replacements — new
  Container IDs — in additions (CP-1, CP-14). *(Form: test)*
- **PK-13.** A modified file whose current Entry is held by a one-file
  Container is eligible for both `freeze` (PK-1) and `update` (PK-11).
  Either path uploads the current local content — `freeze` builds the
  replacement from the local file (PK-7) — and `freeze` additionally
  regroups the file into a Pack. The same overlap holds when the one-file
  Container carries a key-lost marker: either path re-encrypts the surviving
  local plaintext (PK-11). *(Form: test)*
- **PK-14.** Any scan that selects `freeze` or `update` candidates must
  surface every update-eligible file (PK-11) — local content differing from
  its current Entry, or a key-lost Container; neither is ever silently
  skipped, because silent skipping makes the user believe stale or
  unrecoverable content is safely backed up. *(Form: test)*
  - The obligation covers exactly the files the scan considered, which PK-17
    bounds: a file outside the invocation's folder scope is not one it kept
    silent about.
  - It stops the same way at a mapped root the device cannot vouch for: nothing
    under an unavailable root (EP-12) was walked, so those files are outside
    what the scan considered and this rule does not reach them. EP-12 obliges
    the run to report the mapping and the reason instead, so silence never
    leaves the user believing that subtree is backed up.
- **PK-15.** Every user-data Container records one explicit kind:
  **one-file Container** or **Pack**. The kind is not inferred from Entry
  count. Uploading one file on its own creates the former; `freeze`, repack,
  and compaction create the latter. Read-modify-replace for `update` or
  deletion preserves the old Container's kind in its replacement, so a Pack
  left with one Entry remains a Pack and an `update` replacement for a
  one-file Container remains one-file. The replacement has a new Container
  ID and is not the same Container. An oversized singleton Pack is a form of
  Pack, not a third kind. *(Form: test)*
- **PK-16.** The normal fetch unit is a complete Container, not an individual
  Entry. A client may use authenticated range reads to make an Entry available
  early, stream a large Entry, or resume an interrupted transfer, but those
  reads are steps in fetching the containing Container and do not define a
  separate single-Entry fetch operation. *(Form: test)*
- **PK-17.** One `freeze` invocation considers the files under the folders its
  request names. An update-eligible file (PK-11) outside them is outside the
  invocation's scope rather than one it passed over, and PK-14's surfacing
  obligation covers exactly the files the scan considered — a run over another
  folder, or over the Library root, considers the rest. *(Form: test)*
