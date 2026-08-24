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
//! 1. **Scan** (spec: EP-8, EP-9, EP-10). Walk each of the device's mappings
//!    and translate the regular files under it into Entry Paths. A path the
//!    Library holds no current Entry for is new. A path whose file no longer
//!    matches the size and modification time this device last observed is a
//!    *candidate*, settled by hashing the plaintext and comparing it with the
//!    current Entry's hash — equal content is a file that was touched and not
//!    changed. A row this device materialized whose file is gone is a local
//!    deletion. An Entry this device never materialized is outside its scope
//!    and is never reported as changed or deleted, mapping or no mapping.
//! 2. **Decide, and surface what is not decided here** (spec: PK-14). A new
//!    file becomes a one-file Container. A changed file whose current Entry
//!    lives in a one-file Container becomes a replacement Container, the old
//!    one going into the batch's removals (spec: CP-14, PK-12, PK-15). A
//!    changed file whose Entry lives in a Pack, and a file deleted locally, are
//!    reported in [`SyncOutcome::deferred`] and left exactly as they are:
//!    read-modify-replace over a Pack is the half of `update` this flow does
//!    not do (spec: PK-10, PK-11) and propagating a deletion is an explicit
//!    flow of its own. Reporting them is not optional.
//! 3. **Spool** (spec: FM-1 to FM-9, FM-14, OC-2). Encode each Container under
//!    a Container Key of its own, write the ciphertext to a file in the spool
//!    directory, and record it as a pending upload before a byte goes out — the
//!    local provenance that makes cleaning up after an interrupted run possible
//!    at all.
//! 4. **Upload** (spec: FM-3). Put each spool file under the name its Container
//!    ID gives it, through the policy's [`RetryPolicy`](crate::RetryPolicy),
//!    and compare the digest the provider reports for what it stored against
//!    the one taken while writing the spool.
//! 5. **Commit.** Hand the batch to [`commit_batch`](crate::commit::commit_batch),
//!    which is where the Library's state actually changes (spec: CP-1). The
//!    files this run put in place travel with it as
//!    [`PreparedBatch::materialized`](crate::commit::PreparedBatch::materialized),
//!    so the commit's own refresh is what marks them present and clears their
//!    pending rows (spec: EP-10, OC-2).
//! 6. **Reconcile** what an interrupted run left behind (spec: OC-2, OC-3). A
//!    spool is never resumed into a batch: the Container Key that opens it
//!    lived only in the run that drew it, and the one place it would ever have
//!    been written down is the Keyring the interrupted commit never reached
//!    (spec: KD-2, FM-14, KL-7). So the spool is deleted, and this run's own
//!    scan has already spooled the source file afresh. Trashing the object an
//!    interrupted run did upload takes more: an Index caught up to the head,
//!    which only a commit brings about (spec: CK-9). That is why the step runs
//!    last, and why a run that committed nothing leaves such a Container to one
//!    that does.
//!
//! [`sync_folders`] is the whole of the public surface. The steps are private
//! because none of them is a state a caller may stop at: a spooled Container
//! that is never committed is an orphan waiting to be cleaned up, not a result.
//!
//! What is deliberately not here: Pack construction, deletion propagation,
//! `prune`, orphan reclamation on Storage, resumable-upload sessions, MIME
//! detection, and the download path.

mod candidate;

mod deferred;
pub use deferred::Deferred;

mod reconcile;

mod reconciled;
pub use reconciled::Reconciled;

mod run;
pub use run::sync_folders;

mod scan;

mod source_file;

mod spool;

mod spooled;

mod survey;

mod sync_error;
pub use sync_error::{LocalOperation, SyncError, SyncResult};

mod sync_keys;
pub use sync_keys::SyncKeys;

mod sync_outcome;
pub use sync_outcome::SyncOutcome;

mod sync_request;
pub use sync_request::SyncRequest;

mod upload;
