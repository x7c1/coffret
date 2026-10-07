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

Each device also gives a Library a **device-local Library name**, used for its
directory and user interface on that device. It is chosen by the person, is
never written to Storage, and may differ between devices. This is distinct
from the **Library ID**, the random Library-wide value the Library's
recognizable Storage app folder is named after. The device-local name must not
appear in a diagnostic event (spec: EL-1); the Library ID identifies no person
or file and may remain as Storage evidence (spec: EL-5). The names inside a
Library — its Entry Paths and file names — stay out of diagnostic events too,
by the rule the [Entry Path](../entry-path/#domain-rules) concept states.

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
| Has this device materialized its file and not witnessed it go? | Present, or remote |
| Can the committed Keyring supply its Container Key? | An envelope, or a key-lost marker |
| Does this running Library hold its Master Key? | Unlocked, or locked |

These are independent axes. A current Entry can be remote and key-lost: its
name remains in the Catalog, this device has no local file, and its stored
ciphertext cannot be opened. A Library lock describes the running device;
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
| Unresolved condition | A file needing update, an unavailable root, key loss, uncertain pending work, degraded replicas | While a later run still observes the condition |
| This run's result | Completed settlement, a repair performed, failed post-commit trash or checkpoint | As the result of that attempt; later attempts report their own results |

A successful run with unresolved findings has completed its own work while
leaving the reported conditions for attention (spec: PK-14, EP-10, EP-12,
KL-15, OC-2, CK-8).

### Browsing surface

The **explorer** is the browser surface for browsing and operating a Library.
Its **reader** displays an ordered sequence of Entries; a **page** is one step
of that sequence. The explorer itself is also loaded as a page in the
browser, and "the explorer's page" means that browser page, never a step of
the reader's sequence. An Entry is **openable** when the reader supports
displaying it, as determined from its name; every file can be stored whether
or not the reader supports it (spec: FM-9). The **desktop app** is coffret's desktop
application: it opens the Library, serves this surface in the system browser,
and has a window of its own for entering the Passphrase.

## Examples

- A family photo collection: `albums/2024-summer/IMG_0001.jpg`, …
- Scanned books: `books/some-novel/page-001.png`, …
- One Library arranged differently on two devices: a laptop maps only
  `albums/`, a desktop only `books/`; each syncs its own subtree, and each
  one's [Index](../index/) still holds the whole Library's Catalog
- A laptop that maps `albums/` but keeps only `albums/2026/08/` on disk: the
  rest of the album stays in the Library, untouched by the laptop's syncs

## Collocations

- open (a Library on this device, making it available for operations)
- scan (the mapped local folders for new or changed files)
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
  into [Packs](../pack/))
- survey (the files a freeze will pack)
- update (modified local files by replacing their current Containers)
- materialize (an Entry into a file in a [mapped folder](../mapping/))
- add (a file to a mapped folder where no Entry of the Library stands — a
  browser's drop, or the person copying it in — for a later run to carry into
  the Library)
- spool (a Container's ciphertext to a local file before uploading it)
- settle (what an interrupted run left behind, before this one scans)
- stamp (the filesystem identity a mapped root stood on, during a scan)
- stamp (a fetched file with its Entry's own modification time)
- vouch (for a mapped root, as the device — whether the root is there to be
  read from)
- vouch (for itself, as the root — whether the folder standing there is the one
  whose [marker](../mapping/#definition) the mapping recorded)
- refuse (to place into a mapped root that will not vouch for itself)
- surface (a file a run reports rather than silently skips)
- remedy (a refusal, or a state a run keeps reporting, by the gesture its report
  names — renaming a folder, naming a different Entry Path, recording a mapping
  afresh) — the verb that answers *refuse* and *surface*; *resolve* is kept for
  path resolution
- fetch (a folder's files back onto this device) — the Library-side name for
  what the [Pack](../pack/) concept calls `open`: one folder's files arrive by
  fetching the distinct Packs that hold them, and a file somebody asked for
  arrives by a range read over the chunks covering it alone
- serve (a Library for browsing on this device, to a browser on it)
- unlock (a Library served on this device: its [Master Key](../master-key/),
  in place, with the Passphrase entered in the desktop app's own window)
- lock (a Library served on this device, after the idle interval or when its
  server stops)
- drop (files a browser drops into a mapped folder, for a later run — a sync or
  a freeze — to carry them into the Library)
- fill (the folder around an Entry somebody just opened, by fetching in the
  background the rest of what that folder holds and this device has not got)
- arm (a run on this device: ask for it, so the server starts it as soon as
  the runs ahead of it allow)
- supersede (a fill, by arming one for another folder: the earlier fill stops
  between one Entry and the next and is not taken up again on its own)
- displace (a stopped run from the server's record of the last run of its
  kind, as a later run of that kind does; the stopped run is still reported
  beside it, with its refusal)
- discard (queued work, or a scratch or spool this device can safely remove)

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
  - What a device records about a Library it holds — that ID, where on Storage
    the Library is, the device-local name, the account it references, the
    mappings — is the device's own settings, kept on the device and never
    uploaded. So two devices may hold
    one Library under different names, in different folders, and still restore
    the same [Catalog](../catalog/) (spec: EP-9, CK-7).
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
    Library stands — is not materialized: this device holds no record of having
    placed it, and a scan can report it only as new. There are two ways into
    that state. The file may be new, so nothing has uploaded it and nothing has
    fetched it; or its Entry left the Library — another device removed the
    Container it lived in — while the file stayed on disk. Materialization is
    of an Entry, so a file whose Entry left is not materialized either. Either
    way it becomes materialized when a run carries it in, which is the only way
    into the Library (spec: EP-10).
  - A folder standing in a mapped folder with no Entry under it is not a folder
    of the Library, since a folder exists only where a current Entry stands
    under it (spec: EP-2). A listing of the mapped folder gives such a folder
    by name only, apart from the Library's folders, so a page about to make a
    new folder knows the name is already taken by files on disk.
  - A mapped root this device cannot vouch for — missing, or empty while
    standing on a filesystem other than the one recorded for it — is an
    **unavailable root**: the check establishes whether the root is there to be
    read from at all. Nothing under it is walked and no Entry under it is
    reported as deleted; the run reports the mapping and the reason, so an
    unplugged disk or an unmounted share reads as a root to reconnect rather
    than an emptied folder (spec: EP-12).
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
    joining device's Index holds nothing until its first sync or fetch catches
    it up to the current state, which is the same catch-up any device makes
    (spec: CK-9).
  - A join asks whether the place it was given holds any head or
    [Index Snapshot](../index-snapshot/) of a Library, since one that has
    committed anything holds at least one of the two whatever `prune` has
    deleted (see [Journal](../journal/); spec: CK-2, CK-4, CK-6). A place
    holding neither is **nothing yet**, not a failure: a Library created and
    not yet synced looks exactly like this, and it is the Library a second
    device most often joins first. On S3 it is also how a mistyped prefix
    shows itself, since a prefix is not checked the way a Drive folder's name
    is.
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
  Catalog (spec: CP-1, OC-2, OC-7).
  - Settlement reclaims a proven abandoned batch, completes a committed batch's
    interrupted local records, or retains an unknown outcome; exclusive local
    ownership prevents it from reclaiming another live run's work (spec: OC-2,
    OC-3, OC-7).
  - Failed trash keeps the pending provenance so it can be retried safely;
    removing already absent local leftovers is idempotent (spec: OC-2, OC-8).
- A local writer writes its **scratch** — the file it fills before the rename
  that publishes it — inside a mapped folder, which is also a folder a scan
  walks, so coffret reserves a local filename prefix for those files and a
  scan passes over every local name carrying it. A fetch is one such writer,
  and so is an upload the browser drops into a mapped folder. A scratch whose
  rename never comes is the writer's own leftover, and removing one is
  idempotent: one already gone is a successful removal, absence being the
  outcome sought (spec: EP-11, OC-8).
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
- Findings report unresolved conditions and the results of work performed,
  using the lifetimes in the model above; a reported condition is never
  silently treated as backed up or repaired (spec: PK-14, KL-15).
- Failures after the Journal record lands cannot undo the commit; reports of
  failed trash or checkpoint writes let later operations retry the unfinished
  work (spec: CP-1, OC-6, CK-8).
- One `freeze` invocation selects among the files under the folders its request
  names, so an update-eligible file outside them is outside that invocation's
  scope rather than a file it silently passed over — that surfacing obligation
  covers the files the scan considered, and a run over another folder, or over
  the Library root, considers the rest (spec: PK-17, PK-14).

## Technical Constraints

Metadata-only rename is planned. It will commit a change to the Catalog's
names without replacing the affected Containers. Mappings will continue to
translate names to local paths; they do not make local folders the authority
for the namespace. A device following a rename must protect local changes and
retry a refused move without prematurely relabeling its materialization
record. The current implementation does not provide this operation or infer
it from a local file being renamed.

## Related Concepts

- [Container](../container/) — the encrypted unit files are packaged into
- [Entry Path](../entry-path/) — a file's canonical name in the Library
- [Mapping](../mapping/) — how a device lays the Library out over its own
  folders
- [Storage](../storage/) — where the encrypted Library lives
- [Catalog](../catalog/) — the committed shared state
- [Index](../index/) — its cached copy and this device's local records
- [Specification register](../../spec/) — the behavioral rules cited by ID
