# Pack Construction

Rule prefix: `PK`. What makes a Container a Pack, which files `freeze`
selects, how it cuts them into Packs, what its Journal batch contains, how
`update` propagates modified files, how Packs are replaced or removed
when Entries change or are deleted, and how a Container is read back by the
parcel.

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
  - The entry table closes a non-empty Pack too, at a bound short of the
    ceiling a meta section may declare (FM-2), because the size target does not
    imply that ceiling: a selection of very many small files can drive its
    table up to that bound while the target, counting content and table
    together (PK-6), is still far off.
    Segmentation is the only step that can act on that bound — by the time
    the layout refuses such a table the cut is already made, and repeating the
    freeze cuts it the same way — and cutting early costs only more and
    smaller Packs, which PK-4 already admits.
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
- **PK-9.** Deleting Entries — named individually by Entry Path, or as a
  folder and everything under it — examines every current Container holding a
  deleted Entry: a Container whose Entries are all deleted is removed, and a
  Pack that also contains kept Entries is replaced by read-modify-replace
  (PK-10). One deletion commits as one Journal batch: the removed Containers
  and the replaced Packs in removals, the replacements in additions (CP-1,
  CP-14). *(Form: test)*
  - A one-file Container holds one Entry, so deleting that Entry removes it.
  - Because Pack path ranges can overlap across invocations, the number of
    mixed Packs is not bounded by two.
  - Under the initial policy every mixed Pack is normal, with its pre-padding
    footprint capped by the target; an oversized singleton cannot be mixed
    because it contains only one Entry.
  - Deletion is not undoable in the Library: a removed Container ID is never
    added again (CP-14). The removed objects go to the provider's trash
    (OC-6), which keeps their ciphertext only until it is purged, and putting
    an object back from there restores neither its membership in the current
    set nor its key — the Keyring stops mapping a Container once it is
    removed (KL-7).
- **PK-10.** Read-modify-replace reads and verifies every Entry in the old
  Pack, carries each unchanged Entry forward, substitutes each changed Entry,
  and omits each deleted Entry. If any old Entry cannot be read and verified,
  the writer must not commit the replacement; if no Entry remains, the old
  Pack is removed without creating an empty replacement. *(Form: test)*
  - A carried Entry keeps what the old Pack recorded for it — its Entry Path,
    `original_mtime`, `original_btime`, and hash (FM-9) — and the carried
    Entries keep their order. The replacement is encrypted under a new
    Container Key (KD-2).
  - A writer that cannot read and verify the old Pack commits nothing for it:
    neither the replacement nor the old Pack's removal. Every Entry of the old
    Pack, the ones it was asked to delete included, stays current, and the
    refusal is reported naming the Pack. Other Containers in the same
    operation are unaffected.
  - A Pack mapped to a key-lost marker (KL-7) cannot be read, so a deletion
    that would keep some of its Entries is refused for that Pack and reports
    the Entries it keeps; no partial copy of an unreadable Pack is made.
    Deleting all of its Entries needs no read and removes it (PK-9, KL-17).
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
- **PK-16.** The fetch unit is the parcel (PK-19), not an individual Entry
  and not necessarily the whole Container. Every read of a Container's chunks
  asks for whole parcels — one parcel, several adjacent ones, or the whole
  object, every parcel at once — and no range smaller than a parcel is issued,
  not even to show an Entry early. *(Form: test)*
  - Showing an Entry early needs no smaller read: the parcel is streamed,
    every chunk authenticates on its own (FM-5), and the Entry is released as
    soon as the chunks covering it have arrived, while the rest of the parcel
    is still on its way.
  - An Entry that spans a parcel boundary is reached by reading every parcel
    it overlaps. Which parcels those are follows from the header and the entry
    table alone (FM-2, FM-9), so a reader names them before any chunk arrives.
  - The front of the object — its header and meta section (FM-2) — is not part
    of any parcel. It is read on its own, is the same read for every Entry of
    the Container, and names no Entry.
  - A range read holds what came back against the extent it asked for: an
    answer of any other length — a provider that ignored the range and sent the
    whole object, or one that stopped short — is refused rather than decoded,
    and every chunk inside the range still authenticates on its own (FM-5,
    FM-7, FM-8). A range the object's own plaintext stream does not reach names
    no chunk and is refused as out of bounds rather than aimed at the end of
    the object.
- **PK-17.** One `freeze` invocation considers the files under the folders its
  request names. An update-eligible file (PK-11) outside them is outside the
  invocation's scope rather than one it passed over, and PK-14's surfacing
  obligation covers exactly the files the scan considered — a run over another
  folder, or over the Library root, considers the rest. *(Form: test)*
  - A request may also name an explicit selection of Entry Paths, and then the
    invocation considers only the selected paths under its folders. The
    selection narrows the folder scope and never widens it: a selected path
    outside the folders, or outside every mapping (EP-9), is not considered.
    Being selected makes no file eligible — a selected path whose current Entry
    a Pack holds, or one outside the device's scope (EP-10), is not packed
    (PK-1, PK-2).
  - A drop that asks for a Pack arms a `freeze` whose selection is exactly the
    Entry Paths that drop wrote. A file already in the destination folder that
    the drop did not write — a one-file Entry from before the drop, eligible by
    PK-1 — is outside that invocation's scope and stays in its own Container.
  - A retry of a stopped drop's `freeze` asks for that drop's selection again.
    Where the server kept none — a server started since the run stopped, say — the
    retry asks for the folder, with no selection.
- **PK-18.** A Pack's entry table is fixed before any of its content is
  written. The layout puts the meta section ahead of the chunk sequence (FM-2,
  FM-9), so `freeze` declares every selected Entry's path, size, and hash from
  the segmentation it just performed (PK-3) and then streams the member files
  through — which is what lets a Pack larger than memory be written at all.
  Declaring is not trusting: the writer counts and hashes each member's bytes
  as they pass, and a member that is not the file the table promises stops the
  Pack instead of being committed under a table that does not describe it.
  *(Form: test)*
- **PK-19.** A Container's chunk sequence (FM-2, FM-5) is divided, from its
  first chunk, into **parcels** of `n` consecutive chunks, where
  `n = max(1, S div chunk size)`, the chunk size is the one the header records
  (FM-6), and `S` is the constant below. The last parcel holds whatever chunks
  remain, so a Container with no more than `n` chunks is one parcel.
  *(Form: test for the division into parcels; the value of `S` is provisional
  and is set by measurement, as below)*
  - A parcel is a chunk run (FM-5) of fixed length at a fixed place. Its
    boundaries are positions in the chunk sequence and have nothing to do with
    where Entries begin or end.
  - `S` is a constant of this register, the same for every Container and
    recorded in none: provisionally 32 MiB. Its value is set by measuring how
    long the first page of a book takes to appear, because `S` is both the
    granularity the provider observes (PK-20) and the lower bound on that wait
    — 32 MiB at 100 Mbit/s arrives in under three seconds, where 100 MiB
    would take about eight.
- **PK-20.** What the Storage provider observes of a read is the parcel: it can
  tell that parcel `k` of object `X` was read, and when. It cannot tell where
  an Entry begins or ends, nor how large one is, because no read starts or
  stops at an Entry (PK-16, PK-19). *(Form: prose — a claim about what an
  adversarial counterparty can infer, honored by construction through PK-16,
  PK-19, and PK-21 rather than observed by a test)*
  - The limit is `S` against the size of what the Entries group into: where `S`
    is close to the size of one volume of a book, the parcels read still tell
    volumes apart, even though the pages inside them stay hidden.
  - The front of the object (PK-16) shows only that the Container was opened,
    since it is the same read whichever Entry is wanted.
- **PK-21.** A fetched parcel is kept on the device until every Entry with
  bytes in it that the device maps (EP-9) is on the device or witnessed absent
  (EP-10, EP-11), or until its Container leaves the current set, and a parcel
  the device holds is never requested from Storage again, so reading a page a
  second time shows the provider nothing. *(Form: test)*
  - An Entry the device does not map, or maps to no local path it could place
    it at (EP-4), is not waited for. A Pack holds whatever folders it was
    frozen from, so a device mapping only some of them would otherwise keep
    the parcels it shares with the rest for ever — and so would a padding-only
    last parcel, which no Entry ever completes.
  - A Container a later commit replaced or removed holds no current Entry, so
    its parcels are let go by the device's first fetch, catch-up, or deletion
    once its catalog no longer lists the Container (CK-9). Until then they cost
    disk and nothing else: a parcel is held under its Container's ID, so no
    read of another Container ever opens it.
  - A kept parcel is device state, never uploaded: losing it costs a read the
    provider can observe and nothing else. One whose file is gone, or no longer
    authenticates as that parcel (FM-5), is not held; it is said, and asked for
    again whole.
  - Cancelling a fetch and reading ahead happen on parcel boundaries only: a
    cancelled fetch asks for no further parcel and never cuts one short to stop
    at an Entry, and read-ahead asks for the next parcel, never the next Entry.
  - A parcel that did not arrive whole is not held, and it is asked for again
    whole rather than from where it stopped.
- **PK-22.** An Entry taken out of parcels is verified by chunk authentication
  (FM-5, FM-7, FM-8) and by its plaintext hash against the catalog (EP-11).
  The Container's ciphertext hash (CP-11, CP-17) is verified only by a read of
  every parcel; a device that holds some parcels of a Container vouches for the
  Entries it placed from them and for nothing else in that Container.
  *(Form: test)*
  - For such an Entry, chunk authentication and the catalog's hash stand in
    for the ciphertext hash: each chunk's tag under the Container Key is bound
    to the chunk's position and to the header (FM-7, FM-8).
