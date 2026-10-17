# Entry Path

Rule prefix: `EP`. The canonical form of an Entry Path, how paths are
compared, how collisions are surfaced, how local roots map onto the namespace,
what a scan may report about the Entries a device holds, what a fetch may place
into a mapped folder, how a mapped root is identified before anything is placed
into it, and how uniqueness is enforced at the Journal commit.

Concept background: [Entry Path](../../concepts/entry-path/),
[Entry](../../concepts/container/entry/),
[Mapping](../../concepts/mapping/).

## Rules

- **EP-1.** Every Entry Path component is valid Unicode, normalized to NFC
  and encoded as UTF-8. A local filename that is not valid UTF-8 is
  unsupported and causes the scan to report an error rather than skip or
  rename the file. *(Form: test)*
  - The rule has a boundary, and the two sides of it owe different answers.
    Text from outside the Library — a name a scan reads off a disk, the
    top-level component a device's mapping is configured with, a prefix a
    caller narrows a run to, the folder and the file the explorer's URL hash
    names — is normalized on the way in, because which spelling a filesystem
    hands back is its business rather than the user's.
    A path the Library already holds is already in this form, so a reader
    that finds one that is not refuses it as malformed instead of
    normalizing it: composing a stored path on the way back would change
    bytes a digest was taken over, and leave a record decoding to something
    other than what was encoded. A Container whose meta section carries such
    a path does not open, a control object whose entry table carries one does
    not decode, and a device catalog holding one is unreadable and rebuilt
    from Storage (RV-5) rather than migrated.
  - The explorer's URL hash is normalized as the explorer parses it and is
    never refused — a hash that names nothing readable reads as the Library
    root — because it is the browser's own view state rather than an input to
    the server. A `?path=` the explorer sends is such an input, and is refused
    where it arrives (EP-2).
- **EP-2.** An Entry Path is non-empty and relative to the Library root. It
  has no empty, `.`, or `..` component, no leading or trailing `/`, and no
  NUL; `/` is the only logical separator. *(Form: test)*
  - This rule has the same boundary EP-1 has, and the same two answers.
    Text from outside the Library that is not in this shape is refused where
    it arrives — a `?path=` the explorer sends, a prefix a command narrows a
    run to — and the refusal tells whoever typed it which part of the shape
    it fails, because a caller told only that a path was refused has no way
    to find the one component that made it so. A path the Library already
    holds is already in this shape, so a reader that finds one that is not
    treats it as malformed, in each of the ways EP-1 has a reader treat a
    stored path that is not normalized. Nothing is trimmed, resolved, or
    sanitized into a path that would have been admissible — that would be
    coffret inventing a Library position nobody asked for (EP-4).
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
  - A rename (FM-15) names a path, and renames the current Entry standing at
    it and every current Entry under it, each by its own element of the
    record and keeping the rest of its path under the new one. The record
    enumerates them as the catalog the rename was planned against holds
    them, so an Entry added under the path by another writer before the
    rename commits is not renamed (EP-7, rule 8).
  - A rename is refused before anything is written where the state it would
    leave breaks this rule — a new path held by an Entry the same record does
    not rename away — and where it would leave one Entry Path both an Entry
    and a folder of Entries: a new path under a path an Entry the rename
    leaves in place stands at, or with such an Entry under it. No device can
    materialize both (EP-4); a Library already holding such a pair keeps it,
    and a rename does not make one. So is a rename to the same path, a path
    renamed to one under itself while Entries stand under it, and one either
    of whose paths lies under the top-level component reserved for derived
    Entries (FM-15). Every rule FM-15 holds a record's renames to against the
    catalog is asked of the rename then too, so a rename that commits is one
    every catalog applies (RV-5).
- **EP-6.** Before a Journal commit, coffret removes every Entry owned by the
  record's removals from the current path map, then inserts every Entry owned
  by its additions. The commit is rejected if an insertion finds an existing
  Entry Path or if the additions contain a duplicate. *(Form: test)*
- **EP-7.** A writer that loses the Journal commit race rebases onto the new
  head and repeats the same uniqueness check, so two concurrent writes to one
  Entry Path become an explicit conflict rather than last-write-wins (CP-7).
  *(Form: test)*
  - A pending change remembers the Entry it acts on by `(Container ID, entry
    number)` (FM-9) and the current name it was prepared against. The rebase
    examines every record between the batch's base and the new head (CP-4)
    and applies their renames to the pending change by that pair:
    1. A pending addition meets a rename that gave another Entry the path it
       adds an Entry at: conflict, as an addition at that path already is.
    2. A pending update — replacing Container C at its Entry's path — meets a
       rename of that Entry: the update follows the rename, and commits as the
       replacement of the same Entry at its new current path. The new
       Container's captured path stays the name the update was prepared
       under, which is what its meta section records (FM-9, FM-15).
    3. A pending rename meets an update that replaced the renamed Entry's
       Container: if the name the rename was prepared against now names the
       update's new Entry (D, m), the rename is retargeted to (D, m) and
       committed; otherwise conflict.
    4. A pending rename meets a rename of the same Entry: conflict.
    5. A pending rename meets an addition or a rename that now holds its
       destination path: conflict.
    6. A pending removal with a read-modify-replace rebuild (PK-10) meets a
       rename of an Entry the rebuild keeps: the rebuilt Pack is kept, with
       no second upload, and the record carries the kept Entry under its
       renamed current path, its captured path again the name it was rebuilt
       under. A Pack absorbing a one-file Container (PK-7) follows a rename of
       the absorbed Entry the same way.
    7. A pending removal meets a rename of an Entry it removes: the removal
       still applies, since it names the Container, and nothing else changes.
    8. Folder renames are not extended: a pending rename names exactly the
       Entries it was prepared with, and an Entry added under the old folder
       by an intervening commit stays where it is. A pending addition is never
       remapped by prefix.
  - A file a followed update or Pack was drawn from stays at the path it was
    captured from. The commit records it as this device's copy of the Entry
    that took over, waiting to be relocated to that Entry's path (EP-10), and
    not as the Entry at its own path.
- **EP-8.** The prototype scans and reads regular files only. The configured
  mapped root is resolved as the user named it and may itself pass through a
  symbolic link. Every component below that resolved root is then descended
  relative to an open directory handle without following links, for folder
  enumeration and for source reads alike; the final source name is also opened
  without following links and is refused unless its opened handle is a regular
  file. A symbolic link therefore creates no Entry Path for its target, and a
  parent or final name replaced after enumeration cannot redirect hashing,
  encoding, or serving outside the mapped root. Once opened, the same handle is
  retained through the read and supplies its length. *(Form: test)*
  - The validated relative location retains the filesystem's spelling as well
    as the normalized Entry Path. Normalizing a decomposed local name decides
    its Library position (EP-1); it does not invent a different local filename
    for the later read.
- **EP-9.** A device maps each local root either to the Library root or to a
  top-level Entry Path component. It may have at most one Library-root mapping
  and at most one mapping for each top-level component. When both kinds are
  present, a top-level mapping represents that subtree and the Library-root
  mapping represents the remainder. An invalid top-level component is
  rejected before any scan runs. The mappings to local paths are device state
  and are never uploaded. *(Form: test)*
  - The top-level component a mapping is keyed by is that component's NFC
    spelling (EP-1), the same one every Entry Path under it carries. So the
    key a device records a mapping under and the spelling a scan composes its
    Entry Paths from are one string: there is no second spelling for a mapping
    to represent a subtree under, or for a fetch to narrow to and find nothing.
  - Each mapping is its own folder to a scan. A file that leaves one mapped
    root and appears under another is a deletion under the first and a new
    file under the second, never a rename (EP-10): a scan pairs a file that
    went with one that appeared only under the same mapped root.
- **EP-10.** A device's mappings (EP-9) only translate Entry Paths into
  local paths; they do not assert that every Entry under a mapped subtree is
  present on the device. A scan discovers new and modified files under the
  mapped folders, and it reports an Entry as deleted locally only when the
  device itself had materialized it — uploaded it, or fetched it into place
  — and the file is now gone, unless the scan carries it as a local rename
  (below). An Entry the device never materialized,
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
  - A materialization record names the Entry it is a copy of by
    `(Container ID, entry number)` (FM-9), never by the path its file stands
    at, and the device keeps that name current. A record of an Entry that a
    record replaces — removes its Container and adds, at the path the Entry
    stood at, an Entry of a Container that record adds (EP-6): an update, a
    freeze absorbing a file, a rebuilt Pack — names the replacement. A record
    of an Entry an adopted Snapshot no longer holds names the Entry standing
    where its own stood, where that Entry's Container is one the replaced
    catalog did not hold, which a replacement's always is; an Entry that was
    current already is never taken for the record's own. Wherever a record's
    Entry has left the Library and another current Entry holds the path its
    file stands at — a rename brought it there, or an addition after the
    record's own Entry left — the device forgets the record: the file is not a
    copy it placed of that Entry, so a scan reports nothing for it, `update`
    never selects it, and a fetch of that Entry to that path is refused rather
    than overwriting it (EP-11). An Index file written before records named
    their Entry has each record named after the Entry its catalog holds where
    the record stands, or where it waits to follow; a record the catalog holds
    no Entry for there is dropped and the drop reported, never guessed at.
  - A file whose Entry a replayed or adopted rename gave another current path
    (FM-15, RV-5) stays at the path it was placed, under its materialization
    record, and the record notes the Entry's new path. Until the file is
    relocated the record is not the Entry at its own path: a scan neither
    reports the file as deleted or departed nor carries it in as a new file,
    `update` and `freeze` never select it, a deletion does not take it for a
    departed copy, and a fetch to that path treats the file as one this device
    never placed (EP-11). Two Entries that swap names swap what their files
    wait for, and a rename back to where the file stands ends the wait. When a
    later record removes the Entry the wait ends too, and EP-15 decides what
    the file is at the path it stands at, unless an addition of that record
    takes the path the file waited for; an adopted Snapshot that holds no
    Entry at that path ends the wait the same way.
  - A sync, and a folder fetch, **relocate** every file awaiting relocation
    before they scan or place anything: the file is renamed from where it
    stands to the local path the Entry's current path translates to, with one
    filesystem rename that makes the folders the new path needs and never
    replaces anything standing there, and only then does the record follow
    it to the new path, ending the wait. A relocation never uploads,
    re-encrypts, or commits anything. A folder the run's relocations left with
    nothing in it is removed as EP-15 removes the folders its discards empty.
    Files whose Entries traded paths among themselves, which no rename of one
    of them can start, are exchanged into place, each exchange one rename
    that replaces neither file. A fetch of one Entry relocates nothing: it
    reads one file for the person who asked, and what it places at the new
    path is met by the next run as the destination already holding this
    device's copy.
  - A relocation is **refused**, the file left where it stands and the record
    still waiting, and the refusal reported with its reason on every run that
    meets it: where the mapped root holding either path is unavailable
    (EP-12), or stands on another filesystem than the one recorded for it
    until a scan re-stamps it; where the two paths do not lie under one mapped
    root (EP-9), or the new path passes through a name a mapped folder keeps
    for itself (EP-14, EP-16, the scratch prefix of EP-11); where either path
    crosses a symbolic link or anything else that is not a folder of the root
    (EP-8); where the file was edited since it was placed — its length and
    modification time differ from the record's and its content does not hash
    to the hash the record carries (EP-15); where something stands at the new
    path that is not this device's placed copy of the same Entry; where the
    two paths stand on different filesystems, since nothing is copied and
    deleted in a rename's place; where their filesystem offers no rename that
    never replaces, or no exchange, since none is emulated by checking that the
    new path is free and then renaming onto it; and where the root does not
    vouch for itself (EP-13) or the disk refuses. A later run, once the cause
    is gone, relocates the file.
  - Where the new path already holds this device's own copy of the same
    Entry, its record agreeing with the file there (EP-11) — a fetch placed
    it while the old file waited — the old file is redundant: an unedited one
    is discarded into the desktop's trash as EP-15 discards a departed file
    and its record is forgotten, and an edited one is kept where it stands and
    reported as refused for its edit. The record of the new path is the one
    that stands.
  - The record changes only after the file is relocated, so an interrupted
    relocation never leaves a record pointing at a path its file was not
    relocated to. Interrupted before the rename, it leaves the file and the
    record as they were. Interrupted after the rename and before the record
    follows, it leaves the file at the new path agreeing with the old record,
    and no record there: the next run re-keys the record without renaming
    anything.
    Interrupted between an exchange and its two records, it leaves each file
    where the other's record says, agreeing with that record and not with its
    own, and the next run exchanges the records rather than the files.
  - A record whose file is gone from where it stands has nothing to relocate.
    It follows the Entry to its new path where nothing stands there and no
    record does, so a scan reports the Entry deleted locally there as it
    would have at the old path; otherwise it is forgotten, since it describes
    nothing standing at the new path. An absent record follows its Entry the
    same way.
  - A scan **carries a local rename**: within one scan's scope, a file this
    device materialized that is gone is paired with a file that appeared, and
    the pair is a rename rather than a deletion and an addition, only where
    all of these hold. The gone file's record names the current Entry at its
    path and waits for no relocation. The appeared file stands where the
    catalog holds no Entry and the device keeps no record, under the same
    mapped root (EP-9). Its length and content hash are the gone file's
    Entry's; the scan hashes only an appeared file whose length is some gone
    file's Entry's. And the pairing is one to one: exactly one gone file and
    exactly one appeared file share that length and hash, and no file this
    device materialized and still has is recorded holding it. Any ambiguity —
    two gone files with the content, two appeared ones, or one of each beside
    a copy the device still has — pairs nothing, and every file involved is
    what it would be without pairing: the appeared ones new, the gone ones
    deleted locally.
  - A pair becomes one rename (FM-15) in the batch the sync commits: the gone
    file's Entry, named by `(Container ID, entry number)`, takes the appeared
    file's Entry Path. Nothing is uploaded for either file and no Container is
    written; the batch is rebased, or refused as a conflict, like any batch
    carrying renames (EP-7). Once the record lands the gone file's record is
    re-keyed to the new path with its observation unchanged, and waits for
    nothing, since the file already stands there; an interruption between the
    two leaves the case above where the file stands at the new path agreeing
    with the old record, which the next run re-keys. The run reports each
    pair as a rename from the old path to the new one, never silently.
  - A pair is held to every rule a rename the person asks for is held to: the
    reserved namespace, a destination held by an Entry the batch does not
    rename away, a destination that would make a path both a file and a
    folder, and FM-15's rename-set check. A pair one of them refuses is not
    paired: its files are read as without pairing, and the run reports the
    refusal beside them.
  - A gone file that pairs with nothing keeps its record and is reported
    deleted locally, so a later scan whose scope covers its partner can still
    pair them. Pairing changes nothing about what a sync scans.
- **EP-11.** A fetch places an Entry at a local path only where the device can
  vouch for what is there: either nothing at all, or this device's own
  materialization record (EP-10) agreeing with the file on disk. Any other
  state — a file this device never placed, or one whose record and disk state
  disagree — is surfaced as a conflict and never overwritten. An Entry whose
  absence this device already witnessed is not re-fetched either. A fetched file
  becomes visible at its final path only once it is fully verified: every
  chunk it was taken from authenticates and the Entry's plaintext hashes to
  what the current catalog records for it (PK-22), and the bytes reach the
  destination directory as a scratch that is then renamed into place, so no
  reader ever observes a partial or unverified file. Every Entry a fetch
  declines to place is reported with the reason it was declined, on the same
  no-silent-selection posture EP-4 sets. *(Form: test)*
  - A fetch finds the Entry in its Container's entry table by its entry
    number (FM-9) and places the bytes at the Entry's current path. It never
    finds an Entry by path: the table records the path the Entry was captured
    under, which a rename leaves behind (FM-15), so a path lookup would miss a
    renamed Entry or, after two Entries swapped names, find the other one.
    This holds for a whole read and a parcel read alike (PK-16), and for the
    other Entries a parcel read places alongside the one asked for. An entry
    number outside the table is refused (FM-9), with no lookup by path in its
    place, and the bytes found at the number are still held to the hash the
    current catalog records for the Entry.
  - Writes and reads use the same confinement boundary. A fetch creates and
    publishes against directory handles reached below the mapped root without
    following links. A later local read, including the explorer's file route,
    independently descends from that configured root under EP-8 and keeps the
    opened regular-file handle while it streams; it does not reopen a translated
    absolute path after deciding that the Entry is present or after the fetch
    completes.
  - The file a fetch places carries the Entry's own modification time (FM-9),
    stamped on the handle the run wrote to and before the rename, so what
    appears at the final path is already the Entry's in that respect too and
    never a file timed by when it was written. The stamp is also what lets the
    agreement below answer: a file left with the time it was written would look
    changed to every later run. No birth time is stamped onto it.
  - The two states the device can vouch for are exactly the two EP-10 admits: a
    path outside its scope, which it may claim by placing a file there, and one
    it materialized itself, whose file it may replace with the same Entry's
    current content. A file it did not place may be an unsynced source file, so
    overwriting it would destroy content the Library never held.
  - The materialization record and the file on disk **agree** when the path,
    opened without following links (EP-8), is a regular file whose byte length
    equals the recorded length and whose modification time equals the recorded
    one at second precision. That is a change-detection condition — it decides
    whether this device's own placement is still what stands there — and not
    content authentication: a file altered to the same length and modification
    time is not detected, and the rule does not claim otherwise.
  - A folder fetch continues past an Entry it declines and reports each one,
    while a single writer — the upload route the browser drops a file into, or a
    write already in progress — fails as a whole when its one placement is
    declined. A **place** is the local path a fetch resolves an Entry to, so an
    unreachable place and placing an Entry are one word seen twice.
  - A single writer handed several placements at once — the upload route's
    multipart drop — declines each placement that is one file's business and
    reports it beside what it placed, and fails the request as a whole where
    the refusal is a mapping's business rather than a file's (EP-13). Which
    side a placement's refusal falls on is the condition it stands on, not the
    wire kind it is answered with. A mapping's business covers a mapped root
    that could not be asked about at all — the device could not read the
    marker that decides which folder it is — as well as one found not to be
    the folder the mapping was recorded against: neither is anything about the
    file that happened to ask first, and the second one alone says the mapping
    is wrong. This rule reaches no further than placing a file, and the other
    refusals this register gives such a request are stated where they belong:
    a budget the request passes stops it under LA-10, and a volume with no
    room for what is still coming, or that cannot be asked about at all,
    refuses it under LA-11. A body that cannot be read as a multipart drop at
    all is refused as malformed, which no rule here states because it is the
    shape of the request rather than anything about the Library.
  - Where a drop is addressed at the Library root and the device holds more
    than one mapping (EP-9), its parts may go through more than one mapped root,
    and the first refusal that is a mapping's business ends the request —
    including for parts a sound mapping would have taken.
  - A file dropped onto the explorer keeps the person's file's modification
    time where the browser sends it. Each part carries that time as its field
    name: the whole milliseconds from the Unix epoch, in decimal, negative
    before 1970 — what a browser knows of a file as `File.lastModified`. The
    upload route keeps the whole second the moment falls in, truncating toward
    negative infinity so that a time before 1970 stays before it (−1500 ms is
    −2 s), and stamps the file with that second on the handle it wrote to and
    before the rename, as a fetch stamps the file it places; the Entry the next
    sync or freeze makes of the file therefore carries that second as its
    `captured_mtime` (FM-9). A part whose field name is not such a count leaves
    the file with the time it was written at, and is not refused for it.
  - A browser sends no birth time, so the Entry made of a dropped file carries
    no `captured_btime` (FM-9), whatever birth time the file's filesystem
    reports — that one is when the route wrote it. The device notes each file
    it placed this way, with its length and the second it stamped, and the scan
    of the sync or freeze that carries it in records no birth time for a file
    still standing there as the drop left it. The note is not a record of
    materialization: the file is still merely added, and a scan reports it
    only as new (EP-10). The note is held in memory by the open Library, for the
    sync or freeze the drop arms; a file first scanned after the device has let
    it go — the Library was locked, or the process ended, before that run read
    it — is read like any other. A file put into a mapped folder in any other
    way keeps the birth time its filesystem reports.
  - A **scratch** is the file a local writer fills before the rename that
    publishes it. It is written inside a mapped folder, which is also a folder
    a scan walks, so coffret reserves a local filename prefix for it. Every
    local writer publishing by rename into one takes its scratch names from
    that prefix, and a scan passes over every local name carrying it instead of
    reporting it as a file to back up (EP-1, EP-8). A fetch gives its scratches
    no other kind of name, and neither does an upload the browser drops into a
    mapped folder, which is written and renamed for the same reason. A run
    killed between the write and the rename therefore leaves nothing a later
    sync would commit as an Entry. The cost is that anything of the user's own
    carrying that prefix is not backed up — a file, or a folder and everything
    under it, since the scan stops at the name and never looks inside — which
    is the trade for a crash never inventing an Entry out of an interrupted
    fetch.
  - The reserved prefix is `.coffret-fetch-`. A local name is reserved exactly
    when it starts with that string, so a user can tell which names to avoid and
    a scan decides the question from the name alone.
- **EP-12.** Reporting an Entry as deleted locally (EP-10) requires the mapped
  root it stands under to be *available*: the root directory exists, and the
  filesystem it stands on is the one recorded for that mapping (EP-9). A device
  records that filesystem's identity per mapping, stamped by the scan that first
  sees the root and re-stamped whenever a root holding files stands on a
  different one; the identity is device state and is never uploaded (CK-7). A
  mapping whose root is missing, or whose root holds nothing and stands on a
  filesystem other than the recorded one, is unavailable: nothing under it is
  walked, no Entry under it is reported as deleted locally, no file under it is
  selected for `update` or `freeze` or relocated (EP-10), and the run reports
  the mapping and the reason rather than returning silently — an unplugged
  disk or an unmounted share must never read as the user having emptied the
  folder. The device's other mappings scan normally, and a top-level mapping
  that is unavailable still represents its subtree, so the Library-root
  mapping neither walks it nor infers deletions under it (EP-9).
  *(Form: test)*
  - A root that holds files and stands on a filesystem other than the recorded
    one is available and is re-stamped: a device number that moved across a
    reboot or a remount is not evidence that a folder went away. The two
    conditions are asymmetric deliberately — every way the comparison can be
    wrong ends in skipping deletion inference or in re-stamping, never in
    reporting a deletion the recorded identity would not have. The state this
    leaves for a person is a folder genuinely emptied whose filesystem identity
    also moved, and the gesture is recording the mapping again, which clears the
    identity so the next scan stamps what is there.
  - A relocation is stricter about a root than a scan: a root standing on a
    filesystem other than the recorded one is not relocated in whether or not
    it holds files, until a scan has re-stamped it. Relocating a file is a
    write into a folder the device has not yet recognized, and
    waiting one run costs nothing but the wait (EP-10).
  - What the rule does not cover: a mount point whose underlying directory is
    not empty reads as available and is re-stamped, because there is nothing to
    distinguish it from a folder holding files. That is the behavior of a device
    with no recorded identity at all, so the rule never makes such a root worse.
  - A root holding nothing but the device's own management area (EP-14) holds
    nothing here: the comparison looks past `.coffret/`, so the asymmetry above
    is unchanged by the marker's presence (EP-13). A name that only folds to
    that one fits neither half of the asymmetry and is reported instead,
    under EP-14's comparison rather than resolved here.
- **EP-13.** Recording a mapping (EP-9) also records an identity for the root
  folder itself, distinct from the filesystem identity EP-12 stamps: a marker
  file at `<mapped root>/.coffret/root` holding a random identifier, and the
  same identifier kept as that mapping's expected identity in the device's own
  state. That expected identity is device state, like the mappings themselves,
  and is never uploaded (CK-7); the marker file stands inside the device's own
  management area, which a scan never reports as content to back up (EP-14).
  Before placing anything — a fetch (EP-11), an upload received from the
  browser, a sync write — the device opens the configured root as the user
  named it, which may pass through a symbolic link (EP-8), descends `.coffret`
  and then `root` from that open handle without following links, requires the
  first to be a directory and the second a regular file, reads and parses the
  marker, and compares it with the expected identity. Placement then proceeds
  from that same opened root handle, never from a re-resolved path.
  *(Form: test)*
  - The marker's content is the identifier as sixteen lowercase hexadecimal
    characters, optionally followed by one newline, and nothing else. The
    identifier is eight random bytes, spelled the way a Library ID (FM-18) and a
    Container ID (FM-3) are spelled, so one text form is written, read, and
    compared everywhere. A device reads at most 64 bytes of the marker and
    treats anything longer, or shaped otherwise, as malformed.
  - The device **refuses to place** when the root is missing; when `.coffret`
    or `root` is missing, is a symbolic link, or is not the required kind; when
    the marker is malformed or over the cap; when the identifier differs from
    the expected one; and when the mapping has no expected identity recorded at
    all — a mapping read back from a device-state file the device could not
    otherwise use, for instance. A refusal names the mapping and the reason, on
    the no-silent-selection posture EP-4 sets, and propagates the way a declined
    placement does (EP-11): a folder fetch continues past it, while a single
    writer fails that request as a whole.
  - The device may also be unable to ask the question at all: the operating
    system refuses the read that answers it — a permission the process has not
    on `.coffret` or on the marker is the ordinary shape of it. Nothing is
    placed then either, and this is none of the refusals above: the folder was
    not found to be the wrong one, so what is reported names neither the
    mapping nor a reason, and nobody is sent to record a mapping again over a
    permission; what the operating system said travels in their place. What it
    shares with a refusal is its reach, because the marker stands in the root
    everything placed through that mapping goes through and the answer was
    already missing before the first file was written: a single writer handed
    several placements at once fails that request as a whole (EP-11). The
    refusal above also says what a folder fetch does with it; no rule here says
    that for this one, so the two are not to be read alike on that half.
  - Ordinary operation never creates or repairs the marker. A scan, a fetch, an
    upload, and a sync neither create the root, nor `.coffret/`, nor `root`, nor
    rewrite a marker, nor change the expected identity. Only recording the
    mapping does, and it does so conservatively: where `.coffret/` is absent it
    creates the directory and the marker with a fresh identifier and records it;
    where a valid marker already exists it **adopts** that identifier without
    rewriting the file, so several mappings, or several devices, sharing one
    root share one identity and none of them destroys another's; where the
    marker is malformed, over the cap, or a symbolic link, where `.coffret`
    exists without `root` — an interrupted registration — or where `.coffret` is
    not a directory, recording the mapping is an error and writes nothing. The
    interrupted registration is the one of those the name decides rather than
    the handle: what the descent reached may be a folder folding to `.coffret`
    and not `.coffret` itself, and EP-14 says what is reported then.
  - A person who wants a root to carry a new identifier — two roots that ended
    up with the same one after a copy — asks for it explicitly when recording
    the mapping. What the rule states is that issuing a new identity takes an
    explicit request; how the tool spells that request is the tool's.
  - What the rule guarantees and does not: it certifies that the folder the
    device is about to write into is the one that was registered. It does not
    distinguish a faithful copy of that folder from the original, does not
    resist deliberate forgery, and does not vouch for what stands on a different
    mount below the root — a marker check at the root says nothing about a
    filesystem mounted further down. A root whose mount path changed is recorded
    again under its new path. Automatic volume discovery and the operating
    system's volume identifiers are not part of the rule.
- **EP-14.** The name `.coffret`, as a path component at any depth under a
  mapped root, is reserved for the device's own management area. A scan decides
  from the name alone, exactly as it does for the `.coffret-fetch-` prefix in
  EP-11: it never enters a folder of that name and never reports anything under
  it as a file to back up; a fetch, an upload, and a sync never place a file at
  a path carrying that component; an Entry Path carrying it is refused for
  placement and reported. Where mappings overlap — one mapped root standing
  inside another — the management area of the inner root is not content of the
  outer one either. *(Form: test)*
  - The name is compared with ASCII case folded, because a filesystem that
    folds case — APFS as macOS ships it, an exFAT volume on either platform —
    does not tell `.COFFRET` apart from `.coffret`, and an open by name hands
    back a handle that carries no name. The comparison draws two verdicts and
    they are not the same one. **Exactly** `.coffret` is the device's own: a
    scan passes over it in silence and a placement refuses it, both as above.
    Any other spelling that folds to it is **refused and reported** wherever the
    reservation is asked: a scan stops and names the folder rather than stepping
    over it, a placement declines the path the way it declines the reserved
    name, a listing of local files says so rather than leaving the name out, and
    a read of one local file under such a name is declined rather than answered
    with nothing of the person's standing there. Unicode case folding is not
    part of the rule.
  - A refusal tells the two apart wherever what remedies them differs: the
    reserved name is remedied by naming a different Entry Path, and a spelling
    that folds to it may be a folder on the person's own disk, which is remedied
    by renaming that. A folder fetch is where it does not differ — the path it
    declines carries the spelling itself, so no rename on this disk would make
    it placeable — and its one report names both spellings rather than the
    reserved one alone. A single writer draws the line, the path handed to it
    being one somebody just named. What no refusal may do is call the person's
    folder the device's own, or say a scan steps over it.
  - Where a scan names the folder is to whoever asked for the run at a
    terminal. A refusal does not put a local path in front of a browser — the
    same name a diagnostic event may not carry either (EL-1) — so a sync or a
    freeze stopped this way reaches a page as a run that stopped, with no folder
    in it. What a page is told about one path is the other three refusals, and
    they name no name of the person's either: what they carry is the reserved
    name and how the spelling standing there differs from it.
  - The cost is the one EP-11 states for its reserved prefix: anything of the
    user's own under a folder named **exactly** `.coffret` is not backed up.
    Only the exact name is priced this way. `.coffret` has seven letters, so
    folding ASCII case admits 128 spellings; passing over all of them would
    charge that same cost 128 times over with the register stating it once, and
    a person who named a folder `.COFFRET` for their own reasons would find out
    when they needed the files back. They are told instead.
  - Recording a mapping that reaches a folder folding to the reserved name and
    holding no marker does not report the interrupted registration EP-13 names.
    The refusal says which name is standing there and that this filesystem does
    not tell the two apart. The spelling is read where that refusal is composed
    and nowhere else, since the handle the descent holds carries none — and
    reading it there is the one place a root is resolved by path a second time,
    which EP-13 rules out for placement and not for a sentence: nothing is
    written or adopted from it.
    - Recording a mapping is the whole of what this covers. A vouch made while
      placing a file reads no spelling back, so it keeps EP-13's sentence
      whatever name the volume handed it. Something standing at that name that
      is not a directory at all keeps EP-13's sentence too, since the spelling
      changes neither the verdict nor what the person does about it.
- **EP-15.** A file is **departed** when a scan finds it at an Entry Path the
  Library holds no current Entry for, and the device's own materialization
  record for that path (EP-10) says the file is present: the device put an
  Entry's content there, and that Entry has since left the Library — removed by
  this device or by another, and caught up. A departed file is never selected
  for upload, by a sync or by a freeze. A sync decides what it is:
  *(Form: test)*
  - **Unedited** — it still holds what the device last made it match: its byte
    length and modification time equal the recorded ones, or, where either
    differs, its plaintext hashes to the content hash recorded beside the
    materialization. It is discarded into the desktop's own trash — the
    freedesktop.org Trash on Linux, the Finder's on macOS — its record is
    forgotten, so the path is outside the device's scope again, and the run
    reports it as discarded with its Entry Path. A folder below the mapped root
    that the run's discards emptied — it holds nothing at all once they are
    made — is removed, and so is each folder above it that is emptied in turn,
    never the mapped root itself; a folder that was empty before the run, or
    still holds anything the run did not discard, stays.
  - **Edited** — anything else. It is left where it is and reported by every
    run as deleted from the Library and kept because it changed, until the
    person moves or removes it. Moved to another path it is an ordinary new
    file there (EP-10).
  - The device records the content hash of the Entry a file was made to match
    whenever it marks the file present — by uploading it, fetching it, or
    finding it unchanged against its Entry — because the Entry's own hash
    leaves the catalog with the Entry. A record that carries no hash, made by a
    build that did not keep one, judges the file by length and modification
    time alone: a file whose either differs is kept rather than guessed about.
  - The discard is made only under a root the scan found available (EP-12), and
    only after the root has vouched for itself against the mapping's expected
    identity, each folder below it has been entered without following links,
    and the name has been stated as a regular file (EP-8, EP-13). Nothing here
    ever deletes a file outright. A discard that is refused — the root does not
    vouch for itself, the volume has no trash, a permission — leaves the file
    and its record where they are, is reported with what refused it, fails
    nothing else in the run, and is tried again by the next run.
  - A path where a current Entry stands again — another device added a file
    there since — is not departed, and the existing rules apply to it. A path
    whose record says absent, or that has no record at all, is not departed
    either: a file standing there is new (EP-10).
  - A departed path whose file is gone from disk as well holds nothing for a
    finding to be about. Its record is forgotten and nothing is reported: the
    deletion it would otherwise read as (EP-10) is of an Entry the Library no
    longer holds.
  - The device that deletes Entries (PK-9) applies this rule to its own files
    of those Entries right after the deletion's commit, rather than leaving
    them to its next sync: an unedited one is discarded and its record is
    forgotten, the folders those discards emptied are removed, and an edited
    one is kept, each reported with the deletion's outcome, under the same root
    checks and the same handling of a refused discard. Nothing there fails the
    deletion, which has already committed, and whatever it leaves is the next
    sync's to decide as above. Every other device applies the rule at its own
    next sync, once it has caught up.
  - A forgotten record and an absent one are not the same state. Absence is a
    deletion the device witnessed of an Entry it materialized, which a fetch
    declines to put back (EP-11); a forgotten record witnesses nothing, so an
    Entry another device later adds at the path is fetched as any other.
  - What the rule does not do: restore anything from the trash, re-register an
    edited departed file by itself, touch a file the device never materialized,
    or empty the provider's trash on Storage (OC-6).
- **EP-16.** The folders a desktop keeps its trash in at the top of a volume —
  `.Trash`, `.Trashes`, and `.Trash-` followed by one or more ASCII digits —
  are never Library content when one stands as the first component directly
  under a mapped root. A scan decides from the name alone, as it does for
  `.coffret` (EP-14): it never enters such a folder and never reports anything
  under it as a file to back up; a fetch never places a file at a path whose
  first component under the mapped root is one, and reports the Entry with
  that folder as the one on the way that is not a folder of the mapped root
  (EP-11). *(Form: test)*
  - A mapped root may be a volume's own top — a disk mounted at the folder the
    person mapped — and a desktop sending a file to the trash on that volume
    keeps the trash there: `.Trash-<uid>` or `.Trash/<uid>` under the
    freedesktop.org Trash specification, `.Trashes` on macOS, and `.Trash` for
    a home folder mapped as a whole. The discard of a departed file (EP-15)
    lands there too. Without the rule the next sync would carry the trash back into
    the Library.
  - Only the first component under a mapped root, because that is the one
    place a desktop puts them; a folder of the same name anywhere deeper is the
    person's own and is walked as usual. Where one mapped root stands inside
    another, the inner root's trash folders are under the outer root at depth
    and are not covered.
  - Exactly those spellings, compared byte for byte. A desktop writes them so,
    and a spelling that differs in case is a folder somebody named. The cost is
    EP-14's kind: anything of the person's own in a folder of one of those names
    at a mapped root's top is not backed up.
  - An Entry already in the Library at such a path — carried in before this
    rule — stays in the Library until deleted. On the device whose record says
    it put the file there, the scan no longer finds the file, so the Entry is
    reported as deleted locally (EP-10).
  - Whether a root holds nothing (EP-12) is not changed: a trash folder still
    counts as something standing in the root.
