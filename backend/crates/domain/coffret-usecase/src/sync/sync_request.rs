use std::path::{Path, PathBuf};

use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::mapped_roots::MappedRoots;
use crate::object_store::ObjectStore;
use crate::spool::Spool;

/// Everything one run of [`sync_folders`](super::sync_folders) works from.
///
/// The two ports, the epoch's keys, the two halves of this device's own disk —
/// where the ciphertext waits between being encoded and being committed, and the
/// mapped folders the scan reads — and the two values a device supplies rather
/// than derives: what it calls this batch and what its clock says. Which
/// folders are scanned is not among them — that is the device's mappings, which
/// the [`Index`] holds (spec: EP-9), so a caller cannot sync a folder the
/// Library does not know it has.
pub struct SyncRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// Where encoded Containers are written, read back, and removed.
    ///
    /// Every byte this run puts on the device goes through it, which is what
    /// lets a case ask what the run does when a spool cannot be created, cannot
    /// be flushed, or cannot be removed (spec: OC-2, OC-8).
    pub spool: &'a dyn Spool,
    /// The folders this device maps into the Library, as the scan reads them.
    ///
    /// Beside the spool because they are the two halves of one disk: this is the
    /// reading half, and every stat, listing, and file the scan takes goes
    /// through it — which is what lets a case ask what the run does when a root
    /// cannot be stated, a folder cannot be listed, or a source cannot be read
    /// (spec: EP-8, EP-12).
    pub roots: &'a dyn MappedRoots,
    /// The directory encoded Containers wait in until their batch commits.
    ///
    /// It is created if it is not there. Nothing else may write into it: a run
    /// deletes the spools it committed and every one an interrupted run left
    /// behind — every one, because a run writes the pending row naming a spool
    /// before it creates the file, so no file here is ever unnamed (spec: OC-2).
    pub spool_dir: PathBuf,
    /// What this device calls the batch this run produces (spec: OC-2).
    pub batch: BatchId,
    /// What this device's clock says as the run starts.
    ///
    /// Every observation the run writes down is stamped with it, so one run's
    /// bookkeeping stands at one moment rather than at as many moments as it
    /// touched files. Nothing about the Library's correctness rests on it
    /// (spec: CP-7).
    pub now: DeviceTime,
    /// The decisions Storage does not make, for the commit this run ends in and
    /// for the uploads that precede it.
    pub policy: CommitPolicy,
}

impl<'a> SyncRequest<'a> {
    /// A run against `store` and `index`, reading the mapped folders through
    /// `roots` and spooling into `spool_dir` of `spool`, under the default
    /// policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        roots: &'a dyn MappedRoots,
        spool_dir: impl AsRef<Path>,
        batch: BatchId,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            spool,
            roots,
            spool_dir: spool_dir.as_ref().to_path_buf(),
            batch,
            now,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
