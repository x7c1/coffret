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
    caller narrows a run to — is normalized on the way in, because which
    spelling a filesystem hands back is its business rather than the user's.
    A path the Library already holds is already in this form, so a reader
    that finds one that is not refuses it as malformed instead of
    normalizing it: composing a stored path on the way back would change
    bytes a digest was taken over, and leave a record decoding to something
    other than what was encoded. A Container whose meta section carries such
    a path does not open, a control object whose entry table carries one does
    not decode, and a device catalog holding one is unreadable and rebuilt
    from Storage (RV-5) rather than migrated.
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
- **EP-6.** Before a Journal commit, coffret removes every Entry owned by the
  record's removals from the current path map, then inserts every Entry owned
  by its additions. The commit is rejected if an insertion finds an existing
  Entry Path or if the additions contain a duplicate. *(Form: test)*
- **EP-7.** A writer that loses the Journal commit race rebases onto the new
  head and repeats the same uniqueness check, so two concurrent writes to one
  Entry Path become an explicit conflict rather than last-write-wins (CP-7).
  *(Form: test)*
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
  scratch that is then renamed into place, so no reader ever observes a
  partial or unverified file. Every Entry a fetch declines to place is reported
  with the reason it was declined, on the same no-silent-selection posture EP-4
  sets. *(Form: test)*
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
    marker that settles which folder it is — as well as one found not to be
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
  - A root holding nothing but the device's own management area (EP-14) holds
    nothing here: the comparison looks past `.coffret/`, so the asymmetry above
    is unchanged by the marker's presence (EP-13). A name that only folds to
    that one settles neither half of the asymmetry and is reported instead,
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
    system refuses the read that settles it — a permission the process has not
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
    interrupted registration is the one of those the name settles rather than
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
  - A refusal tells the two apart wherever what settles them differs: the
    reserved name is settled by naming a different Entry Path, and a spelling
    that folds to it may be a folder on the person's own disk, which is settled
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
