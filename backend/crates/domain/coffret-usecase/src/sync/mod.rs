//! Carrying a folder on this device into the Library.
//!
//! [`commit`](crate::commit) takes a batch whose Containers are already on
//! Storage and makes it the Library's next committed state. This is what
//! produces such a batch in the first place, and it is the first path that runs
//! end to end: a folder on disk goes in, and Containers another device can open
//! come out.
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Settle** what an interrupted run left behind (spec: OC-2, OC-3, OC-7).
//!    A pending row of this device's own is one of two opposite things, and
//!    which one it is decides what the scan will read, so it is answered before
//!    the scan rather than after the commit. Where any row names an uploaded
//!    object the run catches its own Index up to the Library's head (spec: CK-9)
//!    and reads the verdict off it: a Container no record names is an abandoned
//!    batch, so the object is trashed and the spool and the row go with it; a
//!    Container that *is* current is a commit that landed and whose Index
//!    refresh did not, so the interrupted bookkeeping is completed instead — the
//!    Entries it holds become present (spec: EP-10) and the spool and the row
//!    are dropped. A spool is never resumed into a batch either way: the
//!    Container Key that opens it lived only in the run that drew it, and the
//!    one place it would ever have been written down is the Keyring the
//!    interrupted commit never reached (spec: KD-2, FM-14, KL-7), so whatever
//!    the settled row leaves uncommitted is spooled afresh by the scan below —
//!    the whole file where the batch never committed, and nothing at all where
//!    it did and the file has not changed since.
//! 2. **Scan** (spec: EP-8, EP-9, EP-10). Walk each of the device's mappings
//!    and translate the regular files under it into Entry Paths. A path the
//!    Library holds no current Entry for is new. A path whose file no longer
//!    matches the size and modification time this device last observed is a
//!    *candidate*, settled by hashing the plaintext and comparing it with the
//!    current Entry's hash — equal content is a file that was touched and not
//!    changed. A row this device materialized whose file is gone is a local
//!    deletion. An Entry this device never materialized is outside its scope
//!    and is never reported as modified or deleted, mapping or no mapping. A
//!    mapping whose root the device cannot vouch for — one that is not there, or
//!    one that is empty while standing on a filesystem the mapping does not
//!    record — is *unavailable* (spec: EP-12): nothing under it is walked and no
//!    Entry under it is reported as deleted, because an unplugged disk or a lost
//!    network mount is not the user having emptied a folder. It is reported in
//!    [`SyncOutcome::unavailable`] instead, and the device's other mappings scan
//!    normally. A root that holds files on a filesystem the mapping does not
//!    record is available, and the scan re-stamps the mapping with what it saw.
//! 3. **Decide, and surface what is not decided here** (spec: PK-14). A new
//!    file becomes a one-file Container. A changed file whose current Entry
//!    lives in a one-file Container becomes a replacement Container, the old
//!    one going into the batch's removals (spec: CP-14, PK-12, PK-15). A
//!    changed file whose Entry lives in a Pack, and a file deleted locally, are
//!    reported in [`SyncOutcome::surfaced`] and left exactly as they are:
//!    read-modify-replace over a Pack is the half of `update` this flow does
//!    not do (spec: PK-10, PK-11) and propagating a deletion is an explicit
//!    flow of its own. Reporting them is not optional.
//! 4. **Spool** (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9,
//!    FM-14, OC-2). Encode each Container under a Container Key of its own,
//!    write the ciphertext to a file in the spool directory, and record it as
//!    a pending upload before a byte goes out — the local provenance that
//!    makes cleaning up after an interrupted run possible at all.
//! 5. **Upload** (spec: FM-3). Put each spool file under the name its Container
//!    ID gives it, through the policy's [`RetryPolicy`](crate::RetryPolicy),
//!    and compare the digest the provider reports for what it stored against
//!    the one taken while writing the spool.
//! 6. **Commit.** Hand the batch to [`commit_batch`](crate::commit::commit_batch),
//!    which is where the Library's state actually changes (spec: CP-1). The
//!    files this run put in place travel with it as
//!    [`PreparedBatch::materialized`](crate::commit::PreparedBatch::materialized),
//!    so the commit's own refresh is what marks them present and clears their
//!    pending rows (spec: EP-10, OC-2). That refresh is also the one step after
//!    the record whose failure is not the end of the story: what it would have
//!    written down survives in the pending rows, and step 1 of the next run
//!    completes it (spec: OC-7).
//!
//! [`sync_folders`] is the whole of the public surface. The steps are private
//! because none of them is a state a caller may stop at: a spooled Container
//! that is never committed is an orphan waiting to be cleaned up, not a result.
//!
//! What is deliberately not here. **Pack construction**, which is
//! [`freeze`](crate::freeze): this flow makes one Container per file, and
//! grouping those files into Packs is a separate operation over the same folders
//! (spec: PK-1, PK-7). Also deletion propagation, `prune`, orphan reclamation on
//! Storage, resumable-upload sessions, and MIME detection. The journey back —
//! Containers another device committed becoming files in the folders this device
//! maps — is [`fetch`](crate::fetch), which is this flow's mirror rather than a
//! step in it.

mod candidate;

mod reconcile;

mod reconciled;
pub use reconciled::Reconciled;

mod run;
pub use run::sync_folders;

mod scan;

mod spool;

mod surfaced;
pub use surfaced::Surfaced;

mod survey;

mod sync_error;
pub use sync_error::{SyncError, SyncResult};

mod sync_outcome;
pub use sync_outcome::SyncOutcome;

mod sync_request;
pub use sync_request::SyncRequest;

// What the operating system refused, and the keys one epoch's Containers are
// sealed with, are shared with the [`fetch`](crate::fetch) that goes the other
// way: neither is a fact about the direction the bytes are travelling, so both
// are named once at the crate root and re-exported where their callers already
// reach. A mapped root this device cannot vouch for is shared with
// [`freeze`](crate::freeze) for the same reason: both flows walk the same roots,
// so it is one finding rather than one per flow (spec: EP-12).
pub use crate::library_keys::LibraryKeys;
pub use crate::local_operation::LocalOperation;
pub use crate::unavailable_root::{RootUnavailable, UnavailableRoot};
