use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};
use crate::relocate::LocalDisk;
use crate::relocations::Relocations;
use crate::rename::rename_pair::RenamePair;
use crate::spool::Spool;

/// Everything one run of [`rename_entries`](super::rename_entries) works from.
///
/// The two ports, the epoch's keys, and the pairs of Entry Paths renamed: in
/// each, the one renamed — one Entry, or the folder of every Entry under it —
/// and the one it is renamed to, all of them committed in one Journal record.
/// No batch id: a rename writes no Container, and the [`spool`](Self::spool) it
/// takes is only ever discarded from.
///
/// And, where the device hands them over with
/// [`with_local_disk`](Self::with_local_disk), its mapped folders, the rename a
/// file follows its Entry by, and the trash a redundant copy goes to: then the
/// run relocates this device's files of the renamed Entries right after its
/// commit, as the next sync or fetch would (spec: EP-10). Without them the run
/// touches no mapped folder and those runs relocate them.
pub struct RenameRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// Where this device's Containers waited to be uploaded.
    ///
    /// A batch of this device's whose record the catch-up finds committed and
    /// whose refresh never ran has its bookkeeping completed before the plan,
    /// and its spools are discarded from here (spec: OC-7): until then the
    /// files it carried in have no materialization record, and a rename of
    /// their Entries would leave them where they are rather than relocate them
    /// (spec: EP-10).
    pub spool: &'a dyn Spool,
    /// What is renamed to what, committed together or refused together
    /// (spec: FM-15).
    pub pairs: Vec<RenamePair>,
    /// Where the run says which phase it is in, and how far through the
    /// commit it is.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make, for the commit this run ends in.
    pub policy: CommitPolicy,
    /// When the run is, on this device's clock, for what the completion of a
    /// committed batch records (spec: OC-7).
    pub now: DeviceTime,
    /// This device's disk, where the run is to relocate its files of the
    /// renamed Entries after the commit, and `None` where it is to leave them
    /// to the next sync or fetch (spec: EP-10).
    pub(crate) disk: Option<LocalDisk<'a>>,
}

impl<'a> RenameRequest<'a> {
    /// A run renaming `from` to `to` in the Library in `store` at `now`,
    /// completing a committed batch through `spool`, under the default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        from: EntryPath,
        to: EntryPath,
        now: DeviceTime,
    ) -> Self {
        Self::of_pairs(
            store,
            index,
            keys,
            spool,
            vec![RenamePair::new(from, to)],
            now,
        )
    }

    /// A run renaming every pair of `pairs` in one Journal record, refused as
    /// a whole where any of them is (spec: FM-15).
    pub fn of_pairs(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        pairs: Vec<RenamePair>,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            spool,
            pairs,
            progress: &UNWATCHED,
            policy: CommitPolicy::default(),
            now,
            disk: None,
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

    /// The same request relocating this device's files of the renamed Entries
    /// once its commit has landed, through the folders `roots` reads, the
    /// rename `relocations` makes, and the trash a copy made redundant goes to
    /// (spec: EP-10).
    pub fn with_local_disk(
        mut self,
        roots: &'a dyn MappedRoots,
        relocations: &'a dyn Relocations,
        trash: &'a dyn LocalTrash,
    ) -> Self {
        self.disk = Some(LocalDisk {
            roots,
            relocations,
            trash,
        });
        self
    }
}
