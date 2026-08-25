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
//!    matching the file on disk. Everything else is a finding — a file this
//!    device never placed, one it placed and no longer recognizes, a deletion it
//!    witnessed — reported and left untouched. Nothing is skipped quietly, which
//!    is the same posture EP-4 takes about never silently selecting one of two
//!    files.
//! 4. **Open the committed Keyring** (spec: KL-1, KL-3, KL-6, RV-2, RV-3). The
//!    caught-up checkpoint names the exact replica set the commit behind it
//!    selected, and one valid replica of it carries the whole mapping. A replica
//!    that does not open, or whose mapping is not the one its name promises, is
//!    stepped over: a degraded set still serves a fetch. A generation no replica
//!    of answers is the loss RV-7 names and stops the run.
//! 5. **Fetch each Container once** (spec: PK-16, FM-15). The fetch unit is the
//!    whole Container however many of its Entries are wanted, pulled under the
//!    policy's [`RetryPolicy`](crate::RetryPolicy), and the BLAKE3 of what
//!    arrives is checked against what the Journal record recorded before a key is
//!    unwrapped at all.
//! 6. **Verify** (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9,
//!    KD-2, FM-14, CP-11). The Container Key comes out of the envelope the
//!    Keyring maps this Container to, the decode authenticates every chunk
//!    before its bytes reach a buffer, and each wanted Entry's plaintext hash
//!    is then compared against what the Index says the current Entry hashes to.
//!    Authenticity says the bytes are a coffret object; that comparison says
//!    they are the committed content *this catalog names*.
//! 7. **Place** (spec: EP-4, EP-10, EP-11). The bytes go to a temporary file in
//!    the destination directory, get the Entry's own modification time, and are
//!    renamed onto the final path, so a reader never sees a partial or
//!    unverified file. The Entry is then marked present, which is what puts the
//!    file inside the sync flow's scope from here on.
//!
//! [`fetch_folders`] is the whole of the public surface. The steps are private
//! because none of them is a state a caller may stop at: a Container fetched and
//! not placed is bytes in memory, and a file written and not marked present is
//! one no later run would recognize as this device's own.
//!
//! What is deliberately not here. **Streaming**: the object is fetched whole and
//! decoded in memory, which image-sized Entries make affordable;
//! chunk-to-disk decode, HTTP Range resume of an interrupted fetch, range-read
//! prefetch inside a Pack (spec: PK-16), and a persistent download cache belong
//! to the viewer. **Restoring** a file whose deletion this device witnessed,
//! which is an explicit operation exactly as propagating a deletion is on the
//! sync side. **Keyring repair** (spec: KL-11, KL-13): a degraded set is read
//! through here, never repaired. And MIME detection, thumbnails, and the viewer
//! connection itself.

mod container;

mod fetch_error;
pub use fetch_error::{FetchError, FetchResult};

mod fetch_outcome;
pub use fetch_outcome::FetchOutcome;

mod fetch_request;
pub use fetch_request::FetchRequest;

mod place;

mod run;
pub use run::fetch_folders;

mod select;

mod surfaced;
pub use surfaced::Surfaced;

mod target;

mod translate;

// The keys one epoch's Containers are opened with, and what the operating system
// refused, are shared with the [`sync`](crate::sync) that goes the other way.
pub use crate::library_keys::LibraryKeys;
pub use crate::local_operation::LocalOperation;
