# Pack Construction

Rule prefix: `PK`. Which files `freeze` selects, how it cuts them into Packs,
what its Journal batch contains, and how Packs are replaced or removed when
Entries change or are deleted.

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
