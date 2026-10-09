use std::path::{Path, PathBuf};

use crate::commit::CommitPolicy;
use crate::delete::delete_selection::DeleteSelection;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};
use crate::spool::Spool;

/// Everything one run of [`delete_entries`](super::delete_entries) works from.
///
/// The two ports, the epoch's keys, where on this device the Packs it rebuilds
/// wait for their commit, which Entries to delete, and the two values a device
/// supplies rather than derives: what it calls this batch and what its clock
/// says. No mapped folder: a deletion is a change to the Library, and what
/// becomes of a local file that held a deleted Entry is the next sync's to
/// decide, on this device as on every other (spec: EP-15).
pub struct DeleteRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// Where rebuilt Packs are written, read back, and removed (spec: OC-2,
    /// OC-8).
    pub spool: &'a dyn Spool,
    /// The directory rebuilt Packs wait in until their batch commits — the
    /// same one a sync and a freeze spool into, so that the next sync settles
    /// whatever an interrupted deletion left there (spec: OC-2).
    pub spool_dir: PathBuf,
    /// Which Entries to delete.
    pub selection: DeleteSelection,
    /// What this device calls the batch this run produces (spec: OC-2).
    pub batch: BatchId,
    /// What this device's clock says as the run starts.
    pub now: DeviceTime,
    /// Where the run says which phase it is in, and how far through the
    /// rebuilds, the uploads and the commit it is.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make, for the commit this run ends in
    /// and for the reads and uploads that precede it.
    pub policy: CommitPolicy,
}

impl<'a> DeleteRequest<'a> {
    /// A run deleting `selection` from the Library in `store`, spooling into
    /// `spool_dir` of `spool`, under the default policy.
    ///
    /// Eight of them, and none is one this layer could derive.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        spool_dir: impl AsRef<Path>,
        selection: DeleteSelection,
        batch: BatchId,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            spool,
            spool_dir: spool_dir.as_ref().to_path_buf(),
            selection,
            batch,
            now,
            progress: &UNWATCHED,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request reporting its progress to `progress`.
    pub fn watched_by(mut self, progress: &'a dyn Progress) -> Self {
        self.progress = progress;
        self
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
