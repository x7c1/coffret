# Library

## Definition

**Library** is a set of files a user entrusts to coffret, and the unit
everything else is scoped to: keys, Storage, and restore all operate on one
Library. A user may keep more than one — say one for family photos and one for
scanned books — and separate Libraries share nothing of their own: each has its
own Master Key, Recovery Code, and Index. What two Libraries kept in one
Storage account do share on a device is that account's grant, which the
provider never told apart by Library in the first place (see
[Storage](../storage/)).

A Library has a **Library name**: what a person calls it, the same on every
device that holds it. It is chosen when the Library is created — or, for a
Library a build before names made, given on its first open by a build that
has them — and kept
with the Library on Storage, in its [Catalog](../catalog/) under the
[Master Key](../master-key/), so a device that joins reads it from there
rather than being told one (spec: FM-15, FM-16). A name is anything a person
can read — it is not empty and holds no control character — and two Libraries
may carry the same one. This is distinct from the **Library ID**, the random
Library-wide value that names both the Library's recognizable Storage app
folder and the directory a device keeps the Library in. The Library name must
not appear in a diagnostic event (spec: EL-1); the Library ID identifies no
person or file and may remain as Storage evidence (spec: EL-5).
The names inside a Library — its Entry Paths and file names — stay out of
diagnostic events too, by the rule the [Entry Path](../entry-path/#domain-rules)
concept states.

Before a Library is unlocked nothing of its Catalog can be read, so each
device keeps a copy of the Library name beside the Library and writes it again
whenever it creates, joins, or unlocks the Library. That copy is what a device
lists its Libraries by, and finds one by when a person names it. Until a
Library a build before names put on a device is first unlocked there, it is
listed and found by the name of the directory that build kept it in. Where two
Libraries on one device carry one name, it is their IDs that tell them apart.
A Library a device knows no name for — one joined before its Catalog caught
up — is said to have none and named by its ID as its ID, never with its ID in
the name's place (spec: FM-15).

A Library on a device **references** the Storage account it reaches its
objects through, by that account's **device-local account name**, and holds
in its own directory the **account-cache key envelope**, sealed under a
[purpose key](../purpose-key/) derived from this Library's Master Key, that
opens the account's grant. Unlocking the Library
is therefore what opens the grant (spec: SA-8, SA-9).

The [Catalog](../catalog/) describes the **current Library state** accepted by
a successful [Journal](../journal/) commit. Local folders are a device's working view of
that state, not a second source of truth. They may temporarily differ from it:
for example, editing a local file creates a local change, and that change does
not become part of the current Library state until a sync commits it.

One or more local folders form that working view. A device can map one folder
to the Library root, map folders to top-level components, or combine both —
for example, keeping most of the Library on one disk and `albums/` on another.
The Library root is the root of the [Entry Path](../entry-path/) namespace; it
does not have to correspond to one folder on disk. Every
[Entry](../container/entry/) records its Entry Path relative to that root and
never a device path, so one Library restores onto whatever arrangement of
disks a device happens to have.

## Mental Model

### Shared state and this device

| Question | State |
| --- | --- |
| Does the Catalog contain this Entry? | Current, or outside the current Catalog |
| Has this device materialized its file at its current path and not witnessed it go? | Present, or remote |
| Can the committed Keyring supply its Container Key? | An envelope, or a key-lost marker |
| Does this running Library hold its Master Key? | Unlocked, or locked |

These are independent axes. A current Entry can be remote and key-lost: its
name remains in the Catalog, this device has no local file, and its stored
ciphertext cannot be opened. An Entry a rename gave a new path is remote until
this device relocates its file there: the file left at the old path is
awaiting relocation (below), not the Entry present. A Library lock describes the running device;
it does not change Catalog membership or turn an available envelope into a
key-lost marker (spec: EP-10, KL-7, DK-4).

### Operations and their reports

A **run** is one execution of an operation such as sync, freeze, or fetch. A
[Journal batch](../journal/) is the atomic committed change that a run may
produce. The run also does local work and may upload ciphertext before that
commit, so its progress and the Catalog's state answer different questions.

A **finding** is a report accompanying a run, distinct from an error that
refuses the run. Findings have two lifetimes:

| Kind | Examples | Reported again |
| --- | --- | --- |
| Unresolved condition | A file needing update, an unavailable root, key loss, uncertain pending work, a Keyring short of valid replicas (degraded where a loss is established), an edited departed file kept, a departed file whose discard into the desktop's trash was refused, a file awaiting relocation that could not be relocated | While a later run still observes the condition |
| This run's result | Completed settlement, a repair performed, failed post-commit trash or checkpoint, a departed file discarded into the desktop's trash, a file relocated, a redundant waiting copy discarded | As the result of that attempt; later attempts report their own results |

A successful run with unresolved findings has completed its own work while
leaving the reported conditions for attention (spec: PK-14, EP-10, EP-12,
EP-15, KL-15, OC-2, CK-8).

### Browsing surface

The **explorer** is the browser surface for browsing and operating a Library.
Its **reader** displays an ordered sequence of Entries; a **page** is one step
of that sequence. The explorer itself is also loaded as a page, in the
desktop app's window or in a browser, and "the explorer's page" means that
page, never a step of the reader's sequence. An Entry is **openable** when
the reader supports displaying it, as determined from its name; every file
can be stored whether or not the reader supports it (spec: FM-9). The
**desktop app** is coffret's desktop application: it opens the Library,
serves this surface in a window of its own, and has a separate window of its
own for entering the Passphrase.

The explorer lists the folders of the Library, and beside them the
**prospective folders**: folders the explorer draws that the Library does not
hold yet, each marked as not in the Library yet and each offered as a place a
drop can land. A prospective folder is one of two kinds:

- An **empty mapped prefix**: a top-level component this device maps that no
  Entry stands under yet. The root's listing names it apart from the
  Library's folders, and only this device's mappings are read for it.
- A **prepared path**: a path a person named with "New folder…" inside a
  folder a mapping of this device reaches, so that a drop can be let go over
  it — a series too long for one drop goes in a few volumes at a time under
  one parent. The explorer's page holds it and nothing else does: nothing is
  sent to the server for it, nothing is made on disk, and a reload forgets
  it. "Forget this folder" on its row forgets it sooner, together with the
  paths prepared under it.

Either becomes an ordinary folder of the Library once an Entry under it is
committed. A Library with nothing in it and nothing mapped is started from the
explorer by mapping the root from the banner over it; there is no explorer
control for a sync, since a drop starts one.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …
- One Library arranged differently on two devices: a laptop maps only
  `albums/`, a desktop only `books/`; each syncs its own subtree, and each
  one's [Index](../index/) still holds the whole Library's Catalog
- A laptop that maps `albums/` but keeps only `albums/2026/08/` on disk: the
  rest of the album stays in the Library, untouched by the laptop's syncs

## Collocations

- create (a Library, by drawing its Library ID and writing its first head,
  `head-0`, to [Storage](../storage/) — a [Journal](../journal/) record that
  adds and removes nothing and carries the Library name it is created with, so
  that even the Library's first commit has a head to start from (spec: CP-2,
  FM-15))
- give (a Library its Library name — at creation, or by a later record; never
  *rename*, which is the verb for Entries and folders)
- open (a Library on this device, making it available for operations)
- scan (the mapped local folders for new or changed files, never taking the
  system files an operating system writes beside them)
- deposit (new content into the Library: adding a file, or replacing a
  file's content with content of a different hash, each of which sets the
  deposit time the [Catalog](../catalog/) keeps for that content)
- sync (the Library to Storage)
- reference (a Storage account on this device, by its device-local account
  name)
- join (a Library another device holds, by entering its Recovery Code and
  naming its app folder)
- map (a local folder to the Library root or to a top-level component,
  recording an identity for the folder as it does) — the record it makes is a
  [mapping](../mapping/)
- restore (the current Library state from intact Storage control state)
- salvage (decryptable file contents when Storage control state is incomplete,
  as [Journal](../journal/#domain-rules) defines it)
- freeze ([eligible](../pack/#domain-rules) local files in a folder directly
  into [Packs](../pack/)) — from the command line, from a drop the person chose
  to add as a Pack, or from the explorer's "Pack this folder…" on a folder this
  device maps, once the person has seen its preview
- preview (a freeze of a folder: count what it would select and what it would
  leave out, by the freeze's own scan stopped before a file is read, changing
  nothing; a deletion: count what would leave the Library, the Containers it
  would remove, the Packs it would rebuild with what rebuilding them reads and
  writes, and the Packs it would be refused for, by the deletion's own plan,
  changing nothing in the Library)
- survey (the files a freeze will pack)
- update (modified local files by replacing their current Containers)
- delete (Entries from the Library — named one by one, or as a folder and
  everything under it — by removing the Containers that hold nothing else and
  rebuilding each [Pack](../pack/) that keeps other Entries around them) — from
  the explorer's "Delete…" on a folder or on a file the Library holds, once the
  person has seen its preview
- rename (an Entry, or a folder — every Entry under it, each keeping the rest
  of its path — to a new Entry Path, in one [Journal](../journal/) record that
  names each Entry it renames; no Container is written and no file's content is
  read or uploaded) — from the command line, or by a sync that carries a local
  rename into the Library
- carry (a local rename into the Library, as a sync does when a file this device
  placed is gone and the one file that appeared in its place under the same
  mapped root holds exactly its Entry's content: the sync's batch renames the
  Entry to where the file now stands, and uploads nothing for it)
- materialize (an Entry into a file in a [mapped folder](../mapping/))
- add (a file to a mapped folder where no Entry of the Library stands — a
  browser's drop, an import, or the person copying it in — for a later run to
  carry into the Library)
- discard (a departed file into the desktop's trash, during a sync, or right
  after the commit of the deletion this device made) — never *trash*, which is
  Storage's verb: a [Storage Object](../storage-object/) is trashed; and never
  *move*: no act on an Entry or a file is called that — a rename changes an
  Entry's path, including its folder, and the device holding its file
  relocates it
- relocate (a file awaiting relocation to its Entry's current path, during a
  sync or a fetch and before either scans or places anything, or right after
  the commit of the rename this device made) — one rename on
  this device's disk, under the mapped root it stands in, and its
  materialization record after it; never *move*, for the reason above
- remove (a folder below a mapped root that a run's discards or relocations
  emptied, once every one is made) — never *delete*, which is the Library's
  verb for Entries
- take (the turn at this device's pending work, as a server's sync, freeze or
  deletion does before it asks for the Library; a run armed while another holds
  it waits for the holder)
- take (a drop, as the desktop app does: the regular files among what was let
  go of are held under a **drop handle** for one import, within the lifetime
  and the cap the spec sets, and what it did not take is reported beside them
  with why; and a held drop, as the import naming its handle does, once —
  whatever that request then comes to (spec: LA-26))
- spool (a Container's ciphertext to a local file before uploading it)
- settle (what an interrupted run left behind, before this one scans)
- stamp (the filesystem identity a mapped root stood on, during a scan)
- stamp (a file this device places with a modification time it was given — a
  fetched file with its Entry's own, a dropped one with the one the browser
  sent, an imported one with the one its own filesystem reported when the drop
  was walked)
- vouch (for a mapped root, as the device — whether the root is there to be
  read from)
- vouch (for itself, as the root — whether the folder standing there is the one
  whose [marker](../mapping/#definition) the mapping recorded)
- refuse (to place into a mapped root that will not vouch for itself; any step
  a run cannot take safely, saying why — a freeze, a file that changed after
  its survey; a placement, a mapped folder whose filesystem gives no file lock,
  remedied by keeping the folder on a local disk (spec: OC-8); a deletion, a
  [Pack](../pack/) it cannot rebuild; a rename, a destination another Entry
  holds or a path it would leave both a file and a folder; a commit, when
  another device committed underneath its batch)
- surface (a file a run reports rather than silently skips)
- remedy (a refusal, or a state a run keeps reporting, by the gesture its report
  names — renaming a folder, naming a different Entry Path, recording a mapping
  afresh) — the verb that answers *refuse* and *surface*; *resolve* is kept for
  path resolution
- fetch (a folder's files back onto this device) — the Library-side name for
  what the [Pack](../pack/) concept calls `open`: one folder's files arrive by
  reading, from the distinct Packs that hold them, the
  [parcels](../container/#domain-rules) they overlap, and a file somebody
  asked for arrives by reading the parcels it overlaps, ahead of the rest —
  never by a read that starts or stops where the file does (spec: PK-16)
- serve (a Library for browsing on this device, to a browser on it)
- unlock (a Library served on this device: its [Master Key](../master-key/),
  in place, with the Passphrase entered in the desktop app's own window)
- lock (a Library served on this device, after the idle interval or when its
  server stops)
- drop (files a browser drops into a mapped folder, or the desktop app takes
  by their paths and imports there, for a later run — a sync, or where the drop
  held a folder the person chose to add as a Pack, a freeze of the files
  dropped, together with those a cut-off drop of the same folder received where
  this drop continues it — to carry them into the Library; the system files
  among them are left out and said quietly (spec: PK-17))
- import (the files a drop onto the desktop app's window let go of, walked on
  this device by their paths, held under a one-time **drop handle**, and placed
  into a mapped folder for a later run to carry in, as a browser's drop is;
  a link is neither followed nor taken, and what was not taken is reported,
  not left out — except what an operating system keeps, which is left out and
  said apart (spec: LA-26, LA-27, LA-28, LA-29, EP-17))
- fill (the folder around an Entry somebody just opened, by reading in the
  background the parcels holding the rest of what that folder holds and this
  device has not got — parcel after parcel, never Entry by Entry, and none it
  already holds (spec: PK-16, PK-21))
- arm (a run on this device: ask for it, so the server starts it as soon as
  the runs ahead of it allow)
- supersede (a fill, by arming one for another folder: the earlier fill stops
  between one parcel and the next and is not taken up again on its own)
- displace (a stopped run from the server's record of the last run of its
  kind, as a later run of that kind does; the stopped run is still reported
  beside it, with its refusal)
- discard (queued work, or a spool this device can safely remove)
- remove (a stale scratch — one a killed writer left and no live writer holds —
  under a mapped root a sync walked, during that sync; or a writer's own scratch
  when its write fails) — never *discard*, which a departed file is, into the
  desktop's trash

## Domain Rules

- One Library has one active [Master Key](../master-key/) epoch and one
  [Storage](../storage/) location.
- A Library is named on Storage by its **Library ID**, a random 64-bit value
  drawn when the Library is created: its objects live in one **app folder**
  called `coffret-<library id>`. The ID is independent of the Master Key, so a
  rotation never moves the Library, and it identifies nothing about the user or
  the files — it is what lets several Libraries share one Storage location and
  what a recovering device looks for (spec: FM-18).
  - The ID is configuration a device keeps for the Library and not key
    material, so a [Recovery Code](../recovery-code/) does not carry it — which
    is part of what keeps a code short enough to write down (spec: FM-18,
    KD-11).
  - A device keeps each Library it holds in a directory named after the
    Library ID, so nothing a person chose names anything on the device's disk
    either. A Library a build before Library names put on a device is in a
    directory named after what that device called it; the first time it is
    opened it is given that name — or the one its records already give it,
    where another device named it first — by a record carrying the name and
    nothing else, and moved to the directory its ID names, while no server
    holds it and on a later open where one does. A server moves it before it
    holds it, keeping the directory's name beside the Library until the unlock
    that gives it (spec: FM-15, LA-8).
  - A device holds a Library once. Two directories that hold one Library ID
    — a copy of one, or an earlier build's directory beside the one the
    Library was moved to — are a fault: the Library is listed once, marked as
    needing attention, and not opened by its name or by its ID until one of
    the directories is gone (spec: FM-15).
  - What a device records about a Library it holds — that ID, where on Storage
    the Library is, the copy of its name, the account it references, the
    mappings — is the device's own settings, kept on the device and never
    uploaded. So two devices may hold one Library mapped onto different
    folders and still restore the same [Catalog](../catalog/), name included
    (spec: EP-9, CK-7).
- A local folder maps either to the Library root or to a top-level component
  of the Entry Path namespace, and [Mapping](../mapping/) defines the record
  that says so. A device may have at most one root mapping, and
  each top-level component maps to at most one folder. When both are present,
  a top-level mapping represents that subtree of the Library and the root
  mapping represents the rest. These mappings belong to the device, so another
  device may arrange the same Library differently (spec: EP-9).
- A scan reports an Entry as deleted locally only if this device itself had
  **materialized** it — uploaded or fetched it into a mapped folder — and it is
  gone. Entries the device never materialized, mapped or not, are outside its
  scope rather than missing, so holding part of a Library never removes or
  rewrites the rest (spec: EP-10).
  - A file merely **added** — standing in a mapped folder where no Entry of the
    Library stands, and where this device holds no record of having placed
    anything — is not materialized, and a scan can report it only as new
    (spec: EP-10). It becomes materialized when a run carries it in, which is
    the only way into the Library. A file a browser drops is added too, and so
    is one the desktop app imports. The device keeps a note of the drop with
    the birth time the run carrying the file in is to record — none for a
    browser's drop, the person's file's own for an import where its
    filesystem reported one; that note is not a record of materialization
    (spec: EP-11).
  - A file is **departed** when its Entry left the Library — this device or
    another removed it — while the file this device materialized for it stayed
    on disk. It is not added: the record says this device put it there, and
    carrying it back in would undo the deletion, so no run uploads it. A sync
    discards an **unedited** one — still holding what this device last made it
    match — into the desktop's trash and forgets the record, so the path is
    outside this device's scope again; the device that made the deletion does
    the same for its own copies right after the commit. An **edited** one is the person's change to a
    file the Library no longer has: it stays where it is and every run reports
    it, until the person takes it elsewhere — at another path it is an
    ordinary added file — or removes it. Telling the two apart takes the
    content hash this device records whenever it materializes a file, because
    the Entry's own leaves the Catalog with the Entry (spec: EP-15). A folder below the mapped root that
    those discards leave with nothing at all in it is removed with them, and
    each folder above it emptied in turn — never the mapped root, and never a
    folder that was empty before the run or still holds anything (spec: EP-15).
  - A file is **awaiting relocation** when a rename in the Library has given
    its Entry another path since this device placed it. It is not departed,
    since its Entry is still current, and not added, since the record says
    this device put it there: a scan neither reports it nor carries it in, and
    no run selects it for update or freeze. It stays where it was placed, and
    the record notes where its Entry stands now. A sync or a fetch relocates it
    before it scans or places anything, and the device that made the rename
    relocates its own right after the commit: one rename to the Entry's current
    path, under the same mapped root, and then the record follows, which ends
    the wait. A file that cannot be relocated — its root unavailable; the file
    edited since it was placed; the new path taken by something this device
    did not place, or by its own file of another Entry still waiting there;
    the new path under another mapped root or on another filesystem; a
    symbolic link on the way; a filesystem that offers no rename that never
    replaces — keeps waiting, is reported, and is tried again by the next run;
    nothing is overwritten, copied, or uploaded. Where this device already has
    its own copy of the Entry at the new path, the waiting one is redundant
    and is discarded like a departed file, or kept and reported where it was
    edited. If the Entry leaves the Library first, the wait ends and the file
    is departed instead (spec: EP-10, EP-15, FM-15).
  - A materialization record is about one Entry, named by its Container and
    entry number, and never about whichever Entry its path holds. A
    replacement — an update, a freeze absorbing the file, a rebuilt Pack —
    carries the record over to the Entry that takes its place. Where another
    Entry comes to hold the path the file stands at — a rename brings one
    there, or one is added after the record's own Entry left — the record is
    forgotten rather than read as that Entry's copy: the file is then one this
    device never placed for it, which a fetch declines to overwrite and no scan
    offers as its update (spec: EP-10, EP-11).
  - A folder standing in a mapped folder with no Entry under it is not a folder
    of the Library, since a folder exists only where a current Entry stands
    under it (spec: EP-2). A listing of the mapped folder gives such a folder
    by name only, apart from the Library's folders, so a page can tell a name
    already taken by files on disk from one nothing stands at — and a path is
    never prepared over one. Such a folder is not a prospective folder either:
    a drop let go over it does not make it, since it already stands on disk,
    and the run that carries the drop in carries in what already stands in it
    too. Nor is an empty mapped prefix or a prepared path
    (see [Browsing surface](#browsing-surface)) a folder of the Library until
    an Entry under it is committed.
  - A mapped root this device cannot vouch for — missing, or empty while
    standing on a filesystem other than the one recorded for it — is an
    **unavailable root**: the check establishes whether the root is there to be
    read from at all. Nothing under it is walked and no Entry under it is
    reported as deleted; the run reports the mapping and the reason, so an
    unplugged disk or an unmounted share reads as a root to make available
    again rather than an emptied folder (spec: EP-12).
  - A mapped root that will not vouch for itself — its marker absent, or
    carrying an identity other than the one recorded for that mapping at
    registration — is a **refused root**: the check establishes whether the
    folder standing at the root is the one whose marker the mapping recorded.
    Nothing is placed into it and the run reports the mapping, so a disk that
    came back empty or a folder that merely answers to the registered name is
    never written into. The check is separate from availability and is made
    before a fetch, an upload, or a sync writes anything: a root that is there
    to be read from can still be the wrong folder (spec: EP-13).
- Multiple enrolled devices may write to one Library. Writes are serialized
  at the [Journal](../journal/) commit point, so no device is the permanently
  designated writer (spec: CP-2).
  - A device **joins** a Library by entering its
    [Recovery Code](../recovery-code/) and naming the Library's app folder. It
    then holds the same [Master Key](../master-key/), at the epoch the code
    carries, under a [Passphrase](../passphrase/) of its own — the stored form
    is per device — and it maps its own folders, so it may arrange the Library
    differently from every other device, while its [Index](../index/) holds
    the whole Library's Catalog as every device's does (spec: KD-11, KD-9,
    EP-9, CK-7).
  - A joining device reaches the app folder through a grant it already holds
    wherever one reaches it, and asks the person to consent only when none
    does, so a second Library of an account the device holds costs no second
    consent (spec: SA-8).
  - Joining changes nothing on Storage: the app folder, the
    [Keyring](../keyring/) and the Journal are already the Library's. The
    joining device's Index holds nothing until a run that catches up brings
    it to the current state — the command line's `fetch`, a sync, a freeze,
    or a server's catch-up as it starts or when the explorer asks for one —
    which is the same catch-up any device makes (spec: CK-9).
  - A join asks whether the place it was given holds any head or
    [Index Snapshot](../index-snapshot/) of a Library, since one that has
    committed anything holds at least one of the two whatever `prune` has
    deleted (see [Journal](../journal/); spec: CK-2, CK-4, CK-6). A Library
    holds its first head from the moment it is created, so a place holding
    neither is **nothing yet**, not a failure: a Library an earlier build
    created and never synced looks exactly like this. On S3 it is also how a
    mistyped prefix shows itself, since a prefix is not checked the way a
    Drive folder's name is (spec: CP-2).
- A Library served for browsing on this device is served to this device alone:
  the server listens on loopback only, and as it starts it draws a random
  **server key** and writes it to a file in the Library's directory that only
  the owner's account can read. It answers only a caller that shows that key.
  Reaching the port is not being the owner — the owner's own browser runs other
  people's pages, and a page can aim a request at a loopback port without ever
  reading the answer (spec: LA-1, LA-2, LA-3).
  - A server key belongs to one running server: a new one is drawn at every
    start, so a key that leaked, or a key file a killed server left behind,
    admits nobody once that server is gone (spec: LA-4).
  - The explorer's page never holds the server key, nor any cryptographic
    key. It is served by the **explorer host**, a process of the desktop app
    that forwards the page's requests to the server with the server key
    attached, and the page shows the host a **browser token** of its own
    instead. The page gets that token by exchanging a one-time **launch
    ticket**, which the app hands it in the URL it loads its own window from
    whenever it opens the explorer, and the person does nothing more than open
    it (spec: LA-15, LA-17, LA-21).
    - A browser token lets a page do everything the explorer can do, for one
      Library, until the token expires, is revoked, the host restarts, the
      server restarts, or the Library served changes (spec: LA-18). It is no
      narrower than that: there are no read-only tokens and no roles. It is
      not a key either — it unlocks nothing, and a locked Library stays
      locked to a page holding one until the Passphrase is entered in the
      app's own window.
    - A tab holding no token — a bookmark, a typed address, a session that
      ended — reads nothing of the Library, and says to reload the explorer
      from the app (spec: LA-21).
  - One server at a time serves a Library on a device, and a second start is
    refused rather than taking the first one's place: a second server would
    publish its key over the first one's, leaving that one running and
    admitting nobody. A server that was killed leaves nothing behind that has
    to be cleaned up before the next one starts (spec: LA-8).
  - Size-independence is the Library's own contract: nothing puts a number on
    how large a file may be, an Entry past a [Pack](../pack/)'s size target
    becoming a Pack of its own rather than a file refused (spec: PK-3). An app
    that serves the Library may hold narrower bounds of its own — what one
    request carrying files in may bring, and whether the volume those bytes
    would land on still has room. Those bounds belong to the server rather
    than the Library because the server takes requests on a socket, where a
    caller other than the explorer can send as much as it likes for as long
    as it likes (spec: LA-9, LA-10, LA-11).
- A Library served on a device is locked or unlocked exactly as that device
  holds its [Master Key](../master-key/) (spec: DK-1), whose own rule says when
  a lock comes and what it leaves behind — after the idle interval, or when the
  server stops (spec: DK-1, DK-4, DK-7). A page that asks the server what it is
  doing is told whether the Library is locked, so a page left open over
  plaintext it decrypted can give that plaintext up once the Library locks,
  instead of holding it until its next request is refused (spec: DK-4). A
  Library served from the desktop app is unlocked again in place from the app's
  own window; one served from the command line, by starting its server again
  (spec: DK-1).
- Scanning local folders only discovers local changes. The current Library
  state changes only when a Journal commit accepts them (spec: CP-1).
- A sync catches up, settles interrupted work, scans mapped folders, spools
  and uploads new ciphertext, then commits; only that commit changes the
  Catalog (spec: CP-1, OC-2, OC-7). A freeze settles interrupted work the same
  way before it builds its Packs (spec: OC-9).
  - Settlement reclaims a proven abandoned batch, completes a committed batch's
    interrupted local records, or retains an unknown outcome; exclusive local
    ownership prevents it from reclaiming another live run's work (spec: OC-2,
    OC-3, OC-7).
  - Failed trash keeps the pending provenance so it can be retried safely;
    removing already absent local leftovers is idempotent (spec: OC-2, OC-8).
- A sync, a freeze and a deletion each own this device's pending work for the
  whole of a run, and a competing run is refused (spec: OC-2). Within one
  server the three take **turns** instead: a run armed while another holds the
  pending work waits, and the turns go in the order the runs ask for them — a
  run asks as its flow takes it up, which for a flow already running is once
  the run before it in that flow has finished. What the server reports of its
  running work says which run holds the turn and which runs wait, and a
  waiting run is reported as waiting for the holder (spec: LA-12). Across
  processes there is no queue: of a server's run and a sync or a freeze from
  the command line, whichever comes second is refused at once, as a conflict,
  and goes through when it is run again after the other finishes
  (spec: LA-12).
- A local writer writes its **scratch** — the file it fills before the rename
  that publishes it — inside a mapped folder, which is also a folder a scan
  walks, so coffret reserves a local filename prefix for those files and a
  scan passes over every local name carrying it. A fetch is one such writer,
  and so is a browser's drop or an import into a mapped folder. A scratch whose
  rename never comes is the writer's own leftover, and removing one is
  idempotent: one already gone is a successful removal, absence being the
  outcome sought (spec: EP-11, OC-8).
  - A writer removes its own scratch when its write fails. One a killed writer
    left is removed by the next sync that walks its mapped root: a sync removes
    every scratch no live writer still holds, and leaves alone a name that
    merely carries the reserved prefix without the shape the writers give their
    scratches — the person's own file, or a folder (spec: OC-8, EP-11).
  - A writer locks its scratch for as long as it writes it, which is how a sync
    tells a live writer's scratch from a killed one's, so a mapped folder is
    expected on a local disk. On a filesystem that gives no file lock — a
    network share, mostly — the writer removes the scratch it just made and
    refuses the write, so every placement into that folder is refused, and the
    remedy is keeping the folder on a local disk (spec: OC-8).
  - The cost is that anything of the user's own carrying that prefix is not
    backed up — a file, or a folder and everything under it, since the scan
    stops at the name and never looks inside — which is the trade for a crash
    never inventing an Entry out of an interrupted fetch.
- The device also keeps a **management area** inside each mapped root — a
  folder holding what the device records about that root rather than any of the
  Library's content, the root's own marker among it. The name is reserved at
  any depth under a mapped root and every reader decides from the name alone: a
  scan never enters it, a listing of a mapped folder leaves it out, and nothing
  is ever placed at a path carrying it (spec: EP-13, EP-14).
  - The cost is the one the reserved prefix above carries: anything of the
    user's own under a folder of that name is not backed up.
- A mapped root may be the top of a volume, where the desktop keeps its
  **trash folder** — `.Trash`, `.Trashes`, or `.Trash-` and a number — and
  where the discard of a departed file lands. Such a folder standing as the
  first component directly under a mapped root is never part of the Library,
  decided from the name alone: a scan never enters it, a fetch never places a
  file in it, and a drop or an import is declined there rather than written,
  so a sync never carries the trash back in (spec: EP-16).
  - A folder of the same name anywhere deeper is the person's own and is
    walked as usual; one of those names at a mapped root's top holding the
    person's own files is not backed up, the cost the management area's name
    carries too.
- An operating system writes **system files** beside the person's own —
  `.DS_Store`, `Thumbs.db`, `desktop.ini`, AppleDouble `._` files, a `__MACOSX`
  folder, and at a volume's top folders such as `.Spotlight-V100` or
  `$RECYCLE.BIN`. None of them is ever part of the Library, decided from the
  name alone: a scan never enters or reports one and a sync or a freeze says
  nothing of them, though a freeze's preview counts them among what the freeze
  leaves as it is, where "Pack this folder…" lists them; a drop — a browser's
  or the desktop app's — leaves them out before anything is written and says so
  quietly rather than as a refusal; and a file sent at such a name anyway is
  declined. Keeping them out before they become Entries matters because a
  file inside a [Pack](../pack/) leaves it only with the whole Pack rewritten
  (spec: EP-17, PK-10).
  - The names that stand at a volume's top are left out only there — directly
    under a mapped root, or directly in a dropped folder — and are the
    person's own deeper down. The rest are left out at any depth. Wherever
    one of these names is left out, a file or folder of the person's own
    carrying it — a name beginning `._` included — is not backed up, the cost
    the management area's name carries too (spec: EP-17).
  - An Entry carried in at such a name before the rule stays until it is
    deleted, and a scan does not read the absence of its file as a deletion.
- The Library's current Container set can be restored from the Master Key and
  Storage while the required control state (defined in
  [Storage Object](../storage-object/)) remains intact. A restore brings back
  exactly the Containers that were current, committed removals and
  replacements included; opening every current Container additionally
  requires its reachable Key Envelope, while a key-lost Container remains
  current with unreadable ciphertext (spec: RV-1, RV-2, RV-7).
- If required Journal history or its [Index Snapshot](../index-snapshot/)
  checkpoint is missing, coffret can salvage contents from decryptable
  [Containers](../container/) but cannot prove which candidates are current;
  salvage is not a restore (spec: RV-4).
- `freeze` is a one-time packing operation, not a persistent folder state: it
  leaves no `frozen` flag to restore, and files added later simply become
  eligible for a later invocation
  (spec: PK-1, PK-2, PK-7).
- A `freeze` refuses to pack a file that changed after the **survey** — the
  first pass, which measures each selected file and fixes the Pack's [entry
  table](../container/#domain-rules) before a byte of content is written. A
  file whose length or content moved in between would land under a table that
  does not describe it, so the run stops instead, leaves the Pack in its spool
  for the next run to settle, and the file is simply eligible again next time
  (spec: PK-18, FM-2, FM-5, FM-9, OC-2).
- A scan surfaces every file needing `update` — changed locally, or held by
  a Container whose key was lost — because silently skipping one would make
  the user believe stale or unrecoverable content is backed up
  (spec: PK-14, PK-11).
- A run served on this device that **stopped** says what stopped it: the
  account of its work carries the refusal that stopped it — whatever refused
  the run, be it Storage, a Library locked before the run began, this device's
  own disk, or the worker itself ending without an answer — and a run that did
  not stop carries none. A displaced run keeps the refusal it stopped with.
  This device's cached Catalog is reported on the same terms: it is reported
  **behind** — its last catch-up did not finish, so it may lag the Library —
  exactly when the report carries what stopped that catch-up (spec: LA-12).
  - Reported caught up says only that the last catch-up reached the head when
    it finished. A cached Catalog that a later commit on another device has
    left **stale** — its last catch-up finished, and the head has moved past it
    since — is reported caught up, not behind, until a catch-up or a read
    Storage refuses shows the move (see [Index](../index/#mental-model);
    spec: LA-12, CK-9).
  - The explorer can say the Library's head has moved past the Catalog's
    before anybody catches up, whether the Catalog is stale, behind, or has
    replayed nothing yet: it looks, as its page comes up, as its tab comes
    back to the front, and every few minutes while the tab is shown, at the
    names of the Library's heads, and draws the control that catches up as
    waiting when one is newer than the Catalog's. The look reads no object, needs no key, changes neither the
    Catalog nor the report above, and does not keep the Library unlocked
    (spec: CK-9, FM-12, DK-4).
- Findings report unresolved conditions and the results of work performed,
  using the lifetimes in the model above; a reported condition is never
  silently treated as backed up or repaired (spec: PK-14, KL-15).
- A deletion takes Entries out of the Library in one commit: a Container
  holding only deleted Entries is removed, and a Pack that keeps others is
  replaced by one carrying just those, read back and verified from the old Pack
  first (spec: PK-9, PK-10). A Pack that cannot be read and verified is left
  whole, the Entries asked for included, and reported — so is a key-lost Pack
  that would keep some of its Entries, since nothing can read them back
  (spec: PK-10, KL-17). It is not undoable in the Library: a removed Container
  never comes back under its ID, and the provider's trash keeps its ciphertext
  only until it is purged — putting it back from there restores neither its
  place in the Library nor its key (spec: CP-14, OC-6). A device holding a
  local copy of a deleted Entry discards it into the desktop's trash once it
  has caught up, unless the copy changed since the device last made it match —
  then it keeps it and says so — so the deletion reaches every device's disk
  without any of them losing an edit. The device that made the deletion does
  this for its own copies right after the commit; every other device does it
  at its next sync (spec: EP-15).
- Failures after the Journal record lands cannot undo the commit; reports of
  failed trash or checkpoint writes let later operations retry the unfinished
  work (spec: CP-1, OC-6, CK-8).
- A **deposit** puts new content into the Library, and whether content is new is
  decided by its hash. Adding a file is one, and so is adding a file again at a
  path a deletion emptied; replacing a file's content is one when the new
  content differs from the Entry it replaces. Each sets a new **deposit time**
  for the content now at that Entry Path, stamped once by the device that
  prepares the batch, with its own clock. Nothing else is a deposit: a rename, a
  Pack rebuilt by a deletion, a freeze packing a file whose content the Library
  already holds, and a replacement with the same content — as healing a key-lost
  Entry from its unchanged local file is — carry the time forward, and a restore
  recovers every time as it was. The time says when the Library received the
  content, not when the file was made or last changed — those are the file's own
  times, which travel with it (spec: FM-15, PK-10, PK-11, PK-12, PK-13, RV-1).
  Content salvaged out of a Library and put into one again is deposited anew
  (spec: RV-4).
- A run whose commit is refused because another device committed meanwhile
  reports the refusal and leaves the Library as it was: no record of its batch
  is created, and the batch is dropped. Running it again catches up first, so it
  plans from the Library as that other commit left it (spec: CP-1, CP-4, CP-18).
- A **rename** gives Entries a new Entry Path and changes nothing else: one
  Journal record names each Entry it renames by its Container and entry
  number, with the path it now stands at, and no Container is written, read,
  or uploaded — the Entry's content, its Container, and the path that
  Container captured it under stay as they were (spec: FM-15, CP-1).
  - Renaming a path renames the Entry standing at it and every Entry under it
    as a folder, each keeping the rest of its path under the new one. The
    record enumerates them as the Catalog held them when the rename was
    planned, so an Entry another device adds under the folder meanwhile is not
    swept in and stays where it was added (spec: EP-7).
  - A rename is refused, with nothing committed and nothing uploaded, when the
    path names no current Entry; when the new path is the old one; when either
    lies in the namespace reserved for derived Entries; when a folder would be
    renamed to a path under itself; when an Entry the rename leaves where it is
    already stands at a new path; and when the rename would leave one path both
    a file and a folder. The rules every Catalog holds a record's renames to are
    asked of it before anything is written, so a rename that commits is one
    every device can apply (spec: EP-4, EP-5, FM-15).
  - The device that made the rename relocates its own files of the renamed
    Entries right after the commit; a file that cannot follow is reported as a
    sync reports one, and the rename stands. Every other device's files follow
    on its next sync or fetch, which fetches nothing again for them
    (spec: EP-10).
  - A device whose build predates renames refuses a record carrying them and
    stops there rather than applying part of it, and says a newer build is
    needed (spec: FM-9, FM-15). So does one whose build predates deposit
    times, at the first record it meets, since every record carries them.
    A build refuses a Library created before deposit times in turn; such a
    Library is recreated rather than migrated (spec: FM-15, FM-16).
- A sync **carries a local rename** into the Library, and only where which file
  became which is not a guess (spec: EP-10). A file this device placed is gone,
  a file appeared under the same mapped root where the Library holds no Entry
  and this device keeps no record, the new file's length and content hash are
  the gone file's Entry's, and within the scan no other file — another gone
  one, another new one, or a copy this device still has — holds that content.
  - The pair becomes one rename in the batch the sync commits, and nothing is
    uploaded for it: the Entry takes the new file's path, this device's record
    of the file follows it there, and the run reports `renamed <old> → <new>`
    instead of a deletion and a new file. The rename is held to every rule a
    rename from the command line is, and rebased or refused with the batch as
    any rename is (spec: EP-7, FM-15).
  - Anything short of that pairs nothing, and each file is what it was before:
    a file edited as well as renamed, one renamed into another mapped folder,
    two files with the same content. A pair whose rename the Library would
    refuse is reported with the refusal, and its files are read the same way.
  - A gone file that pairs with nothing keeps its record and is reported
    deleted locally, so a later sync that sees its partner can still pair them.
- A rename another device committed meanwhile is not by itself such a refusal.
  A run's update, Pack, or rebuilt Pack takes over the renamed Entry at its new
  path; only a write to the same Entry or Entry Path from both sides — a rename
  onto a path the run adds, two renames of one Entry, a rename onto a path the
  other side filled — is refused as a conflict (spec: CP-7, EP-7).
- One `freeze` invocation selects among the files under the folders its request
  names, so an update-eligible file outside them is outside that invocation's
  scope rather than a file it silently passed over — that surfacing obligation
  covers the files the scan considered, and a run over another folder, or over
  the Library root, considers the rest (spec: PK-17, PK-14). A request may
  narrow that further to an explicit selection of Entry Paths, which a drop
  added as a Pack does with the files it wrote; a selected file still has to be
  eligible (spec: PK-17, PK-1).

## Technical Constraints

A rename commits a change to the Catalog's names without replacing the
affected Containers: replaying its record or adopting a Snapshot taken after it
gives each Entry its new path, and a fetch finds an Entry in its Container by
its entry number and places it at its current path (spec: FM-15, RV-5, EP-11).
A person asks for a rename from the command line; the explorer does not offer
one yet. Mappings continue to translate names to local paths; they do not make
local folders the authority for the namespace. A file renamed on disk is
carried in as a rename only in the narrow case above, read within one sync's
scan; every other local rename still reads to a sync as one file gone and
another added.

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Entry Path](../entry-path/) — a file's canonical name in the Library
- [Mapping](../mapping/) — how a device lays the Library out over its own
  folders
- [Storage](../storage/) — where the encrypted Library lives
- [Catalog](../catalog/) — the committed shared state
- [Index](../index/) — its cached copy and this device's local records
- [Specification register](../../spec/) — the behavioral rules cited by ID
