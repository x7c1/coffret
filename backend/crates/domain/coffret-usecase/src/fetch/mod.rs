//! Carrying the Library into the folders on this device.
//!
//! [`sync`](crate::sync) goes one way — a folder on this device becomes
//! Containers another device can open — and this is the same journey read
//! backwards: Containers another device committed become files in the folders
//! this device maps. It is the second of the two ways a device materializes an
//! Entry, and EP-10 names them together for that reason: uploaded it, or fetched
//! it into place.
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Catch up** (spec: CK-9, RV-1). Bring the Index to the Library's head
//!    from the newer of two starting points, its own state or the newest valid
//!    checkpoint, and replay the Journal after it. On a device that has never
//!    seen this Library that is a restore followed by a replay, which is what
//!    lets a second enrolled device fetch with an empty catalog; neither step
//!    opens a Container, because a record carries what the Containers it adds
//!    hold (spec: CP-11, RV-5). A fetch that cannot read the head fails rather
//!    than serving a catalog it knows may be stale.
//! 2. **Translate Entry Paths into local paths** (spec: EP-9). Each mapping
//!    says where a subtree's Entry Paths would go locally, and the request's
//!    prefix narrows that — it intersects the mappings rather than replacing
//!    them, because a mapping is what makes a local path exist at all. Between
//!    them the mappings partition the namespace: a top-level mapping represents
//!    its subtree and the Library-root mapping represents the remainder.
//! 3. **Decide, per Entry, whether this device may write there** (spec: EP-10,
//!    EP-11). A fetch places a file only where the local state is one it can
//!    vouch for: nothing there at all, or its own materialization record still
//!    matching the file on disk. The question is asked by descending the mapped
//!    root the way step 7 writes into it, so this is also where a folder that is
//!    not a folder of that root is met. Everything else is a finding — a file
//!    this device never placed, one it placed and no longer recognizes, a
//!    deletion it witnessed, a folder on the way with a shape no file can be
//!    placed through — reported and left untouched, and the run goes on to the
//!    next Entry. Nothing is skipped quietly, which is the same posture EP-4
//!    takes about never silently selecting one of two files.
//! 4. **Open the committed Keyring** (spec: KL-1, KL-3, KL-6, RV-2, RV-3). The
//!    caught-up checkpoint names the exact replica set the commit behind it
//!    selected, and one valid replica of it carries the whole mapping. A replica
//!    that does not open, or whose mapping is not the one its name promises, is
//!    stepped over: a degraded set still serves a fetch. A generation no replica
//!    of answers is the loss RV-7 names and stops the run.
//! 5. **Fetch each Container once** (spec: PK-16, FM-15). The fetch unit is the
//!    whole Container however many of its Entries are wanted, pulled under the
//!    policy's [`RetryPolicy`](crate::RetryPolicy) and decoded as it arrives —
//!    a Pack is sized in gigabytes (spec: PK-5), so nothing here holds more than
//!    a transfer buffer and one chunk of plaintext. The BLAKE3 of what arrives
//!    is checked against what the Journal record recorded, which is a claim
//!    about the whole object and so is settled once the last byte has passed
//!    and before anything becomes visible.
//! 6. **Verify** (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9,
//!    KD-2, FM-14, CP-11). The Container Key comes out of the envelope the
//!    Keyring maps this Container to, every chunk is authenticated before any of
//!    its bytes reach a file, and each wanted Entry's plaintext hash is then
//!    compared against what the Index says the current Entry hashes to.
//!    Authenticity says the bytes are a coffret object; that comparison says
//!    they are the committed content *this catalog names*.
//! 7. **Place** (spec: EP-4, EP-10, EP-11). The destination folder is descended
//!    to from the mapped root one component at a time, refusing to pass through
//!    anything that is not a real folder of that root
//!    ([`LocalPlace::descend`]); the bytes then go to a temporary file *in that
//!    open folder*, are flushed to the device, get the Entry's own modification
//!    time, and are renamed onto the final name, so a reader never sees a
//!    partial or unverified file. The Entry is then marked present, which is
//!    what puts the file inside the sync flow's scope from here on.
//!
//!    The descent is not decoration. An Entry Path comes from another enrolled
//!    device and says nothing about the shape of this device's disk, so a
//!    component that is an ordinary folder there may be a symbolic link out of
//!    the mapped root here — and a writer that joined the components onto the
//!    root would follow it and place bytes somewhere the Library never pointed.
//!    EP-4 refuses a path this device cannot materialize rather than inventing a
//!    place for it, and EP-11 places bytes only where the device can vouch for
//!    what is there; the scan side keeps the mirror of the same discipline by
//!    not following links out of a mapped folder (spec: EP-8).
//!
//! # One Entry, without its Container
//!
//! [`fetch_entry`] is the same journey with step 5 done differently. A reader
//! that wants one page out of an unfetched book must not wait for the gigabyte
//! around it, and it does not have to: a Container says where everything in it
//! is before any of it arrives, so the front of the object plus the chunks
//! covering that one Entry is the whole read (spec: FM-2, FM-5, FM-9). Every
//! other step is the folder fetch's — the catch-up, the mappings, the vouching,
//! the Keyring, the temporary file and the rename.
//!
//! Per PK-16 that is an optimization inside fetching the containing Container
//! and not a fetch unit of its own: the rest of the Container is exactly as
//! unfetched afterwards, and a range read cannot check the object's own hash,
//! because that is a claim about bytes it deliberately did not ask for. What
//! holds over a range is per-chunk authentication for the bytes that arrive
//! (spec: FM-5, FM-8) and the Entry's plaintext hash against the catalog before
//! the file becomes visible (spec: CP-11, EP-11).
//!
//! [`fetch_folders`] and [`fetch_entry`] are the whole of the public surface
//! that moves bytes. The steps are private because none of them is a state a
//! caller may stop at: a Container read and not placed is temporary files, and a
//! file written and not marked present is one no later run would recognize as
//! this device's own.
//!
//! [`local_path_of`] and [`local_place_of`] expose step 2 for a single Entry,
//! and move nothing. The first answers where a file belongs for display and
//! collision checks; the second keeps the mapped root and validated relative
//! location separate for a confined read (spec: EP-8, EP-9). Both use the same
//! translation a fetch performs before placing the file.
//!
//! [`local_path_for`], [`local_place_for`], and [`local_folder_for`] are that
//! same step asked of a path the Library holds no Entry at, which is what
//! something *adding* a file has to ask: where a file dropped into a folder
//! goes, and which folder on this device the folder somebody is looking at even
//! is. They are here rather than wherever such a writer lives for the reason
//! above — one rule, one implementation — and they move nothing either.
//!
//! [`LocalPlace`] is what both readers and writers ask for. Their confinement
//! needs the mapped root and the components below it kept apart, so each access
//! descends rather than handing a joined string to a filesystem. It is public,
//! with [`DescentError`], because the explorer taking a dropped file into a
//! mapped folder is the second writer into these folders and must not grow a
//! second reading of EP-4 and EP-11. What the descent hands back is a
//! [`Destination`](crate::Destination) of the
//! [`Destinations`](crate::Destinations) capability — the folder held open — and
//! that capability is where every call on a filesystem behind step 7 lives.
//!
//! What is deliberately not here. **Resuming** an interrupted fetch from the
//! bytes it had already verified, and filling in the rest of a Pack one Entry
//! was read out of — both are the viewer's prefetch machinery, and both are
//! about scheduling reads rather than about what a read means. **Restoring** a
//! file whose deletion this device witnessed, which is an explicit operation
//! exactly as propagating a deletion is on the sync side. **A download cache**
//! beyond the placed files themselves. **Keyring repair** (spec: KL-11, KL-13):
//! a degraded set is read through here, never repaired. And MIME detection,
//! thumbnails, and the viewer connection itself.

mod container;

mod decoding;

mod entry_fetch;
pub use entry_fetch::EntryFetch;

mod entry_request;
pub use entry_request::FetchEntryRequest;

mod entry_run;
pub use entry_run::fetch_entry;

mod fetch_error;
pub use fetch_error::{FetchError, FetchResult};

mod fetch_outcome;
pub use fetch_outcome::FetchOutcome;

mod fetch_request;
pub use fetch_request::FetchRequest;

mod local_place;
pub use local_place::LocalPlace;

mod local_folder;
pub use local_folder::LocalFolder;

mod placement;

mod range_read;

// What every read of a Container in one run is made against, which is the same
// five things whichever of the two ways it is read.
mod reading;

mod run;
pub use run::fetch_folders;

mod scatter;

mod select;

mod surfaced;
pub use surfaced::Surfaced;

mod target;

mod translate;
pub use translate::{
    local_folder_for, local_path_for, local_path_of, local_place_for, local_place_of,
};

// The keys one epoch's Containers are opened with, and what the operating system
// refused, are shared with the [`sync`](crate::sync) that goes the other way.
// The descent's own refusal is the `Destinations` capability's vocabulary and is
// re-exported for the same reason: a caller of `LocalPlace::descend` reaches for
// the rest of the fetch's words here.
pub use crate::descent_error::DescentError;
pub use crate::library_keys::LibraryKeys;
pub use crate::local_operation::LocalOperation;

/// How much of a transfer is held at once.
///
/// The bytes are handed to the chunk reader as they arrive rather than
/// accumulated, so this is a transfer buffer and nothing else: what a fetch
/// spends is this, one chunk of plaintext, and the file handles it is writing —
/// never the object, which for a Pack is measured in gigabytes (spec: PK-5).
/// Both ways of reading a Container spend the same, because both read one that
/// does not fit in memory.
const TRANSFER_BUFFER: usize = 128 * 1024;
