use std::path::{Path, PathBuf};

use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`sync_folders`](super::sync_folders) works from.
///
/// The two ports, the epoch's keys, where the ciphertext waits between being
/// encoded and being committed, and the two values a device supplies rather
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
    /// The directory encoded Containers wait in until their batch commits.
    ///
    /// It is created if it is not there. Nothing else may write into it: a run
    /// deletes the spools it committed and the ones an interrupted run left
    /// behind (spec: OC-2).
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
    /// A run against `store` and `index`, spooling into `spool_dir`, under the
    /// default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool_dir: impl AsRef<Path>,
        batch: BatchId,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
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
