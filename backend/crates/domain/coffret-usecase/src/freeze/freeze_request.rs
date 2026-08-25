use std::path::{Path, PathBuf};

use coffret_model::EntryPath;

use crate::commit::CommitPolicy;
use crate::device_state::{BatchId, DeviceTime};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;

/// Everything one run of [`freeze_folder`](super::freeze_folder) works from.
///
/// The two ports, the epoch's keys, where the ciphertext waits between being
/// encoded and being committed, which part of the Library to freeze, how large
/// the Packs should come out, and the two values a device supplies rather than
/// derives: what it calls this batch and what its clock says.
pub struct FreezeRequest<'a> {
    /// Where the Library's objects live.
    pub store: &'a dyn ObjectStore,
    /// This device's catalog of the Library.
    pub index: &'a dyn Index,
    /// The keys of the epoch the Library is in.
    pub keys: &'a LibraryKeys,
    /// The directory encoded Packs wait in until their batch commits.
    ///
    /// It is created if it is not there. Nothing else may write into it: a run
    /// deletes the spools it committed, and the sync flow deletes the ones an
    /// interrupted run left behind (spec: OC-2).
    pub spool_dir: PathBuf,
    /// The folder to freeze, or `None` for everything the mappings cover.
    ///
    /// It narrows the run and never widens it: a prefix outside every mapping
    /// selects nothing, because a mapping is what puts a local file at an Entry
    /// Path at all (spec: EP-9). The Library root is the degenerate case, and
    /// nothing about the Packs it produces differs — segmentation is local to
    /// whatever one invocation selected either way (spec: PK-8).
    pub prefix: Option<EntryPath>,
    /// How large a Pack should come out, in bytes before padding (spec: PK-5,
    /// PK-6).
    ///
    /// A pack-policy parameter and not a format constant, which is why it is
    /// here rather than in `coffret-format`: a Library can be repacked under a
    /// different one, and what value serves best is a measurement question about
    /// upload and retrieval behavior, rewrite amplification, object count, and
    /// provider API overhead.
    ///
    /// It is a target and not a maximum. Entries are indivisible, so one larger
    /// than this forms an oversized singleton Pack (spec: PK-3), and
    /// authentication tags and Padmé padding carry every stored Pack somewhat
    /// past what was measured (spec: PK-6).
    pub target: u64,
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

impl<'a> FreezeRequest<'a> {
    /// A run against `store` and `index`, spooling into `spool_dir`, covering
    /// everything the mappings cover, under the default policy.
    pub fn new(
        store: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a LibraryKeys,
        spool_dir: impl AsRef<Path>,
        target: u64,
        batch: BatchId,
        now: DeviceTime,
    ) -> Self {
        Self {
            store,
            index,
            keys,
            spool_dir: spool_dir.as_ref().to_path_buf(),
            prefix: None,
            target,
            batch,
            now,
            policy: CommitPolicy::default(),
        }
    }

    /// The same request narrowed to one folder of the Library.
    pub fn under(mut self, prefix: EntryPath) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// The same request under a different policy.
    pub fn with_policy(mut self, policy: CommitPolicy) -> Self {
        self.policy = policy;
        self
    }
}
