use std::path::{Path, PathBuf};

use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};
use crate::spool::Spool;

/// Everything one run of [`repack_folder`](super::repack_folder) works from.
///
/// The two ports, the epoch's keys, where on this device the staged copies and
/// the new Packs wait, which folder to repack, how large a Pack should come
/// out, and the two values a device supplies rather than derives: what it
/// calls this batch and what its clock says.
pub struct RepackRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// Where staged copies and new Packs are written, read back, and removed
    /// (spec: OC-2, OC-8).
    pub spool: &'a dyn Spool,
    /// The directory they wait in — the one a sync, a freeze and a deletion
    /// spool into, so that the next sync settles whatever an interrupted
    /// repack left there (spec: OC-2).
    pub spool_dir: PathBuf,
    /// The folder whose Packs are repacked, or `None` for the Library root.
    ///
    /// Not optional in the sense a freeze's prefix is: the root is named on
    /// purpose, because a repack of it rewrites every Pack of the Library not
    /// already in place (spec: PK-23).
    pub folder: Option<EntryPath>,
    /// How large a Pack should come out, in bytes before padding — the
    /// parameter a freeze cuts under, cut by the same rule (spec: PK-3, PK-5,
    /// PK-6).
    pub target: u64,
    /// What this device calls the batch this run produces (spec: OC-2).
    pub batch: BatchId,
    /// What this device's clock says as the run starts.
    pub now: DeviceTime,
    /// Where the run says which phase it is in, and how far through the
    /// Packs, the uploads and the commit it is.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make, for the commit this run ends in
    /// and for the reads and uploads that precede it.
    pub policy: CommitPolicy,
}

impl<'a> RepackRequest<'a> {
    /// A run repacking `folder` of the Library in `store` into Packs of about
    /// `target` bytes, spooling into `spool_dir` of `spool`, under the default
    /// policy.
    ///
    /// Nine of them, and none is one this layer could derive.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        spool_dir: impl AsRef<Path>,
        folder: Option<EntryPath>,
        target: u64,
        batch: BatchId,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            spool,
            spool_dir: spool_dir.as_ref().to_path_buf(),
            folder,
            target,
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
