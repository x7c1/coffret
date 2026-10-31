use std::path::{Path, PathBuf};

use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::local_scan::{ScanScope, UnknownBirths, NONE_PLACED};
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::object_store::ObjectStore;
use crate::progress::{Progress, UNWATCHED};
use crate::relocations::Relocations;
use crate::spool::Spool;

/// Everything one run of [`sync_folders`](super::sync_folders) works from.
///
/// The two ports, the epoch's keys, the four things this device's own disk is
/// asked for — where the ciphertext waits between being encoded and being
/// committed, the mapped folders the scan reads, the trash a deleted Entry's
/// unedited copy goes to, and the rename a renamed Entry's file follows it by —
/// and the two values a device supplies rather
/// than derives: what it calls this batch and what its clock says. Which
/// folders are scanned is not among them — that is the device's mappings, which
/// the [`Index`] holds (spec: EP-9), so a caller cannot sync a folder the
/// Library does not know it has. A caller can only narrow the walk to some of
/// those folders ([`within`](Self::within)), which a sync a drop armed does
/// (spec: EP-10).
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
    /// The desktop's trash, which a file a deleted Entry left behind unedited is
    /// discarded into (spec: EP-15).
    ///
    /// A capability like the two above it, so a case asks what the run does
    /// when the trash will not take a file without reaching the trash of the
    /// desktop it runs on.
    pub trash: &'a dyn LocalTrash,
    /// Where a file this device placed is relocated to follow a rename of its
    /// Entry, before the scan (spec: EP-10).
    ///
    /// A capability for the reason the trash is one: what a relocation
    /// promises is about a rename that is refused or interrupted.
    pub relocations: &'a dyn Relocations,
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
    /// Where the run says which phase it is in, and how far through the
    /// encoding, the uploading and the commit it is.
    ///
    /// The first two are where a sync of a large folder spends its minutes —
    /// one step per file and then one per Container. [`UNWATCHED`] is the
    /// default and costs nothing.
    pub progress: &'a dyn Progress,
    /// The decisions Storage does not make, for the commit this run ends in and
    /// for the uploads that precede it.
    pub policy: CommitPolicy,
    /// The files this device placed with no birth time of their own, which the
    /// scan records none for (spec: FM-9, EP-11).
    ///
    /// Only a drop onto the explorer records a file here. The default holds
    /// nothing.
    pub births: &'a UnknownBirths,
    /// Which of the mapped folders the scan walks (spec: EP-10): every one
    /// by default, and the folders a drop wrote into for a sync that drop
    /// armed.
    ///
    /// The scan alone is narrowed. The catch-up, the settling of what an
    /// interrupted run left and the relocation of files waiting to follow
    /// their Entries run as they do for every sync (spec: CK-9, OC-2,
    /// EP-10).
    pub scope: ScanScope,
}

impl<'a> SyncRequest<'a> {
    /// A run against `store` and `index`, reading the mapped folders through
    /// `roots`, discarding departed files into the desktop's trash through
    /// `trash`, relocating renamed Entries' files through `relocations`, and
    /// spooling into `spool_dir` of `spool`, under the default policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool: &'a dyn Spool,
        roots: &'a dyn MappedRoots,
        trash: &'a dyn LocalTrash,
        relocations: &'a dyn Relocations,
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
            trash,
            relocations,
            spool_dir: spool_dir.as_ref().to_path_buf(),
            batch,
            now,
            progress: &UNWATCHED,
            policy: CommitPolicy::default(),
            births: &NONE_PLACED,
            scope: ScanScope::Mappings,
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

    /// The same request, knowing which files were placed with no birth time
    /// of their own.
    pub fn knowing(mut self, births: &'a UnknownBirths) -> Self {
        self.births = births;
        self
    }

    /// The same request, walking only the folders `scope` covers.
    pub fn within(mut self, scope: ScanScope) -> Self {
        self.scope = scope;
        self
    }
}
