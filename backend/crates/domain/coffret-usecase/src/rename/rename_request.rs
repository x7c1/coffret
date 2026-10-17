use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};
use crate::relocate::LocalDisk;
use crate::relocations::Relocations;

/// Everything one run of [`rename_entries`](super::rename_entries) works from.
///
/// The two ports, the epoch's keys, and the two Entry Paths: the one renamed
/// — one Entry, or the folder of every Entry under it — and the one it is
/// renamed to. No spool and no batch id: a rename writes no Container.
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
    /// The Entry Path renamed: an Entry, a folder of Entries, or both where
    /// the Library holds an Entry at a path it also holds Entries under.
    pub from: EntryPath,
    /// The Entry Path it is renamed to.
    pub to: EntryPath,
    /// Where the run says which phase it is in, and how far through the
    /// commit it is.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make, for the commit this run ends in.
    pub policy: CommitPolicy,
    /// This device's disk, where the run is to relocate its files of the
    /// renamed Entries after the commit, and `None` where it is to leave them
    /// to the next sync or fetch (spec: EP-10).
    pub(crate) disk: Option<LocalDisk<'a>>,
}

impl<'a> RenameRequest<'a> {
    /// A run renaming `from` to `to` in the Library in `store`, under the
    /// default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        from: EntryPath,
        to: EntryPath,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            from,
            to,
            progress: &UNWATCHED,
            policy: CommitPolicy::default(),
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
