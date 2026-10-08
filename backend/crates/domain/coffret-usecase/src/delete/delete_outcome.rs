use coffret_model::{ContainerId, EntryPath};

use crate::commit::{CommitOutcome, DegradedKeyring};
use crate::delete::rebuilt_pack::RebuiltPack;
use crate::delete::refused_pack::RefusedPack;

/// What one deletion took out of the Library, rebuilt, and left alone.
///
/// [`commit`](Self::commit) says what became of the Library, and a run that
/// had nothing to commit carries `None` there rather than an empty batch
/// (spec: CP-1). [`refused`](Self::refused) is the half a caller must not skip:
/// a run that returns successfully with refusals in it has *not* deleted every
/// Entry it was asked to, and the Entries of each refused Container are all
/// still in the Library (spec: PK-10).
///
/// Removing a Container is not undoable in the Library: its Container ID never
/// comes back (spec: CP-14), and the object goes to the provider's trash only
/// until the provider purges it. Putting the ciphertext back from there
/// restores neither its membership in the Library nor its key.
#[derive(Debug)]
pub struct DeleteOutcome {
    /// The Entries that left the Library, in Entry Path order (spec: EP-3).
    pub deleted: Vec<EntryPath>,
    /// Their total length, in plaintext bytes (spec: FM-9).
    pub bytes: u64,
    /// The Containers removed outright, every Entry of each named
    /// (spec: PK-9, CP-14).
    pub removed: Vec<ContainerId>,
    /// The Packs replaced by read-modify-replace (spec: PK-9, PK-10).
    pub rebuilt: Vec<RebuiltPack>,
    /// The Containers the deletion was refused for, and why (spec: PK-10,
    /// KL-17).
    pub refused: Vec<RefusedPack>,
    /// Named Entry Paths that held no current Entry.
    pub missing: Vec<EntryPath>,
    /// What the commit did, or `None` when the run had nothing to commit.
    ///
    /// The removed objects are moved to the provider's trash after the commit,
    /// and one that would not go is reported here in
    /// [`untrashed`](CommitOutcome::untrashed) — the deletion stands either way
    /// (spec: CP-1, OC-6).
    pub commit: Option<CommitOutcome>,
    /// The committed Keyring set the run's read had to step over a position
    /// of, where nothing later in the run spoke for it (spec: KL-15).
    pub degraded: Option<DegradedKeyring>,
}

impl DeleteOutcome {
    /// How many Entries left the Library.
    pub fn entries(&self) -> usize {
        self.deleted.len()
    }

    /// How many bytes the rebuilds read from Storage.
    pub fn rebuild_read(&self) -> u64 {
        self.rebuilt.iter().map(|pack| pack.read).sum()
    }

    /// How many bytes the replacements the rebuilds wrote weigh on Storage.
    pub fn rebuild_written(&self) -> u64 {
        self.rebuilt.iter().map(|pack| pack.written).sum()
    }
}
