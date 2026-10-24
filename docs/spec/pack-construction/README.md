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
  - Each Entry of the old Pack is verified against the catalog's row with the
    same entry number (FM-9) on what a rename cannot change — its captured
    path, extent, and hash — and never on its current path, so a Pack holding
    a renamed Entry is rewritten like any other (FM-15).
  - A carried Entry keeps what the old Pack recorded for it —
    `captured_mtime`, `captured_btime`, and hash (FM-9) — and the carried
    Entries keep their order. It is recorded under its current Entry Path,
    which becomes its captured path in the replacement: a rewrite captures the
    name current at the rewrite, so the catalog no longer holds a captured
    path apart from the current one (FM-15). The replacement is encrypted
    under a new Container Key (KD-2).
  - A carried Entry also keeps its deposit time: the replacement's record
    gives it the `deposited_at` the catalog holds for the Entry it carries,
    because a rewrite puts no content into the Library (FM-15). A substituted
    Entry carries new content and is a deposit, stamped once as its batch is
    prepared (FM-15). A repack or a compaction carries its Entries the same way.
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
  - A deposit is new content, decided by its hash (FM-15): an `update`'s
    replacement whose `hash` differs from the Entry it replaces is stamped with
    a new deposit time as its batch is prepared, and one with the same
    `hash` — as re-encrypting a key-lost Container's surviving local plaintext
    gives (KL-17) — keeps the replaced Entry's time. The unchanged Entries a
    read-modify-replace carries keep theirs too (PK-10).
- **PK-13.** A modified file whose current Entry is held by a one-file
  Container is eligible for both `freeze` (PK-1) and `update` (PK-11).
  Either path uploads the current local content — `freeze` builds the
  replacement from the local file (PK-7) — and `freeze` additionally
  regroups the file into a Pack. The same overlap holds when the one-file
  Container carries a key-lost marker: either path re-encrypts the surviving
  local plaintext (PK-11). *(Form: test)*
  - The two paths agree on the deposit time of a modified file: its content is
    new to the Library, so the Pack's Entry is a deposit, stamped as an
    `update`'s replacement would be (PK-12, FM-15). A one-file Entry a freeze
    absorbs whose local file still holds the Entry's content — a key-lost one
    re-encrypted from an unchanged file included (KL-17) — puts no new content
    into the Library: the Pack's Entry carries that Entry's deposit time
    forward, as a Pack rewrite carries its kept Entries' (PK-10) and as an
    `update` with the same content does (PK-12). Whether the content is the
    Entry's is the hash's to say (FM-9).
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
  - A range read is one request for one byte range of the object. It checks
    what came back against the extent it asked for: an answer of any other
    length — a provider that ignored the range and sent the whole object, or
    one that stopped short — is refused rather than decoded, and every chunk
    inside the range still authenticates on its own (FM-5, FM-7, FM-8). A range
    the object's own plaintext stream does not reach names no chunk and is
    refused as out of bounds rather than aimed at the end of the object.
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
    retry asks for the folder, with no selection; the drop's own reservation, below,
    is what takes its files up without being asked.
  - A drop that asks for a Pack records that choice on the device before the
    first of its files lands: a **reservation** of the folder it was made onto,
    naming each Entry Path the drop writes before that file lands, so no file of
    the drop stands in a mapped folder without one. It is device state like a
    mapping or a pending row (EP-9, OC-2) — never uploaded and in no Index
    Snapshot (CK-7) — and it is the drop's record, not folder state of `freeze`'s
    (PK-2). A drop that is refused as a whole, or that lands nothing, arms no
    `freeze` and keeps no reservation it made.
  - A reservation is **received** once its drop has stopped receiving with
    every file it brought: the drop marks it so before it arms its own
    `freeze`. Until then it is a **drop in progress** — one still being
    received, or one **cut off** because the process taking the drop stopped
    part way — and names the files that had landed, not the book. A
    reservation recorded before the device kept the mark counts as cut off.
  - A drop that asks for a Pack onto a folder holding a cut-off reservation
    takes that reservation over instead of making one: its paths are the
    drop's from the start, a file the drop writes again replaces the one on
    disk, and the `freeze` the drop arms once it finishes receiving has a
    selection of every path the reservation names, so the files of both drops
    come to one book. A drop that took one over and is then refused as a whole
    keeps it a drop in progress; one every file of which is refused still
    arms the `freeze` of the files the cut-off drop received.
  - Where the folder holds several cut-off reservations — drops of it cut off
    one after another, or together — the drop takes over the earliest made and
    folds each later one into it: the later one's paths are added to the
    earliest, and then the later one is released. So the drop's paths from the
    start are every cut-off drop's, and the folder is left with one drop in
    progress rather than one per drop that was cut off. A process that stops
    part way through folding leaves a path named by two reservations, which
    the next drop of the folder folds again.
  - A sync's scan selects no reserved path for addition or update, and reports
    each as waiting for its Pack rather than passing over it (PK-14). A reserved
    file is still a file the walk found, so nothing is inferred deleted for it
    (EP-10). Without this a process that stopped between the drop and its commit
    would leave a book a sync carries in one Container per page.
  - A `freeze` that considered every path of a reservation — each inside its
    scope, and none under a mapped root it could not vouch for (EP-12) — decides
    it. Where the run packs any of those paths, the reservation is released by the
    commit's refresh, in the same step that completes the batch's rows (OC-2);
    where none of them is eligible any more — gone from disk, already held by a
    Pack, or ineligible under PK-1 or EP-10 — it is released once the run has
    finished, with a finding per path saying why. A reservation some of whose
    paths are gone packs the rest. A run that stops before either keeps every
    reservation as it was, for the next run to decide. Only a received
    reservation is decided: no `freeze` decides a drop in progress, nor selects
    any path it names, so neither the pages landed so far nor the ones still to
    come are packed apart from the rest of their book. A received reservation
    that names no path is decided only by the `freeze` a server takes it up
    as.
  - While the drop that made or took over a reservation is still receiving, the
    reservation is also **live** in that process: held in memory and nowhere
    else, so a process started afterwards holds none live. No `freeze` decides
    one that was live at any moment since the run read the reservations — what
    it read may lack the files that landed after — and no second drop takes a
    live one over. A drop's reservation is live from the moment it is made:
    making it and holding it live are one step against a second drop of the
    folder looking for one to take over, so of two drops of one folder arriving
    together exactly one holds a given reservation, and the other takes over
    another or makes its own.
  - The drop's own `freeze` is armed once it stops receiving, and is a run of its
    own: it is never taken for a run already under way, even one whose scope
    covers every file the drop wrote, because that run read the reservations
    while the drop was live. It may join a run of the same folder that is still
    waiting its turn, since nothing has walked the folder for that one yet.
  - A server takes every received reservation up once its start-up catch-up
    has finished, and again once the Library is unlocked in place, in the order
    they were made: each as a `freeze` whose selection is exactly its paths,
    taking its turn as any `freeze` does (LA-12). A reservation live in that
    process is not taken up: its drop arms its own run. A cut-off reservation is
    not taken up either: it is kept in place, its files are listed as received
    and waiting for their Pack, and the server says as it starts how many such
    drops are in progress, until a drop of their folder takes each over.
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
    again whole. A parcel file no row names is discarded when a server next
    opens the Library.
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
